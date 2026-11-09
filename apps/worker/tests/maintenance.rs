// P2.20 — Worker-level tests for the partition-maintenance job: the pure
// partition math (no database), the scheduled pass mechanics (pre-creation
// ahead of the ingest frontier), and the retention pass behaviour — the
// recorded D-R4 no-retention-configured state, the D-R2 approval gate, and
// the D-R3 out-of-window-only detach with audit records.
//
// Gated on `FLEXCORE_TEST_DATABASE_URL` (the P1.13 harness convention);
// skips cleanly when unset (CI provides the service). Requires a
// multi-thread tokio runtime (the passes drive the sync adapter ports).
//
// Shared-database courtesy (recorded): the tests use their OWN fixed months
// (2019 m05/m06) for the retention scenarios — never the months the P2.06
// or P2.09 suites create. The approved retention pass is broad by design
// (it detaches EVERY partition entirely outside the window, including ones
// other suites left attached), so the test RESTORES any partition it did
// not itself create by re-attaching it with its captured bounds afterwards.

use std::sync::{Arc, Mutex, MutexGuard, OnceLock};

use flexcore_postgres_adapter::partition_maintenance::{
    is_telemetry_partition_name, month_add, parse_partition_bounds, partition_name,
    plan_pre_creation, retention_cutoff, PartitionEntry,
};
use flexcore_postgres_adapter::{FlexCoreStore, SystemClock};
use flexcore_worker::maintenance::{
    pre_creation_pass, retention_pass, MaintenanceSettings, RetentionStatus, MAINTENANCE_ACTOR,
};
use sqlx::PgPool;

// --- Harness helpers -----------------------------------------------------------

fn test_url() -> Option<String> {
    std::env::var("FLEXCORE_TEST_DATABASE_URL")
        .ok()
        .map(|url| url.trim().to_string())
        .filter(|url| !url.is_empty())
}

fn require_db(test: &str) -> Option<String> {
    match test_url() {
        Some(url) => Some(url),
        None => {
            println!(
                "skipped {test}: FLEXCORE_TEST_DATABASE_URL is not set (CI provides a \
                 PostgreSQL 18 service)"
            );
            None
        }
    }
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

fn run_key() -> &'static u64 {
    static RUN_KEY: OnceLock<u64> = OnceLock::new();
    RUN_KEY.get_or_init(|| {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("the system clock is after the Unix epoch")
            .as_nanos() as u64
    })
}

/// Serializes the database-gated tests in this binary: the approved
/// retention pass is broad by design (it detaches every out-of-window
/// partition), so concurrent passes within the same binary would race on
/// each other's fixture partitions.
fn db_lock() -> MutexGuard<'static, ()> {
    static DB_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    DB_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn test_runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("test runtime")
}

/// The maintenance store: the migrator/maintenance role, with BYPASSRLS
/// granted by the bootstrap (the recorded owner/ops path for the
/// platform-level audit record, P1.12 §5.2 — see the adapter test file for
/// the same bootstrap).
fn ensure_test_roles(pool: &PgPool) {
    let result = test_runtime().block_on(async {
        sqlx::query(
            "ALTER ROLE flexcore_migrator WITH LOGIN PASSWORD 'flexcore_migrator_test_only' \
             BYPASSRLS",
        )
        .execute(pool)
        .await?;
        Ok::<(), sqlx::Error>(())
    });
    result.expect("the maintenance role must be grantable from the admin connection");
}

fn maintenance_store(url: &str) -> FlexCoreStore {
    FlexCoreStore::connect(&url_as(
        url,
        "flexcore_migrator",
        "flexcore_migrator_test_only",
    ))
    .expect("maintenance store")
}

fn attached_names(store: &FlexCoreStore) -> Vec<(String, Option<(i64, i64)>)> {
    let unit = store.begin(None).expect("unit of work");
    let entries = unit.list_telemetry_partitions().expect("listing");
    unit.commit().expect("commit");
    entries
        .into_iter()
        .map(|entry: flexcore_postgres_adapter::partition_maintenance::PartitionEntry| {
            (
                entry.name,
                entry
                    .bounds
                    .map(|bounds| (bounds.from_utc.unix_timestamp(), bounds.until_utc.unix_timestamp())),
            )
        })
        .collect()
}

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

fn relation_exists(pool: &PgPool, name: &str) -> bool {
    test_runtime().block_on(async {
        sqlx::query_scalar::<_, bool>("SELECT to_regclass($1) IS NOT NULL")
            .bind(format!("public.{name}"))
            .fetch_one(pool)
            .await
            .expect("to_regclass query")
    })
}

/// Re-attaches a partition this test did not create (shared-database
/// courtesy): the detach is undone with the captured bounds so the other
/// suites' partitions survive a maintenance test run.
fn reattach(pool: &PgPool, name: &str, bounds: (i64, i64)) {
    let from = time::OffsetDateTime::from_unix_timestamp(bounds.0).expect("from");
    let until = time::OffsetDateTime::from_unix_timestamp(bounds.1).expect("until");
    let ddl = format!(
        "ALTER TABLE telemetry ATTACH PARTITION {name} \
         FOR VALUES FROM ('{} 00:00:00+00') TO ('{} 00:00:00+00')",
        from.date(),
        until.date()
    );
    test_runtime().block_on(async {
        sqlx::query(&ddl).execute(pool).await.expect("re-attach DDL");
    });
}

// --- 1. Pure partition math (no database) -----------------------------------------

#[test]
fn partition_math_and_planning_are_pure_and_total() {
    // Month arithmetic across year boundaries, both directions.
    assert_eq!(month_add(2026, 11, 1), (2026, 12));
    assert_eq!(month_add(2026, 12, 1), (2027, 1));
    assert_eq!(month_add(2026, 1, -1), (2025, 12));
    // Names follow the D-P2 convention and the validator agrees.
    assert_eq!(partition_name(2026, 11), "telemetry_y2026m11");
    assert!(is_telemetry_partition_name(&partition_name(2026, 11)));
    assert!(!is_telemetry_partition_name("telemetry_y2026m1"));
    // Bounds parse strictly; anything else is None (fail-safe).
    let (from, until) = parse_partition_bounds(
        "FOR VALUES FROM ('2026-11-01 00:00:00+00') TO ('2026-12-01 00:00:00+00')",
    )
    .expect("canonical rendering");
    assert_eq!((from.year(), u8::from(from.month())), (2026, 11));
    assert_eq!((until.year(), u8::from(until.month())), (2026, 12));
    assert!(parse_partition_bounds("FOR VALUES IN ('x')").is_none());
    // The planner skips existing months and covers exactly N months.
    let existing = vec![PartitionEntry {
        name: partition_name(2026, 11),
        bounds: None,
    }];
    let plan = plan_pre_creation(&existing, 2026, 11, 2);
    let names: Vec<&str> = plan.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(names, vec!["telemetry_y2026m12", "telemetry_y2027m01"]);
    // The retention cutoff for a 2-month horizon in November 2026 is
    // 2026-09-01 (D-R3: entirely-outside-the-window semantics).
    let cutoff = retention_cutoff(2026, 11, 2).expect("cutoff");
    assert_eq!((cutoff.year(), u8::from(cutoff.month())), (2026, 9));
    // The settings carry the configuration source and the actor (D-R4/D-R5).
    let settings = MaintenanceSettings::from_config(2, None, false);
    assert_eq!(settings.retention_config_source, "FLEXCORE_RETENTION_NORMALIZED_TELEMETRY_MONTHS");
    assert_eq!(settings.actor, MAINTENANCE_ACTOR);
    assert_eq!(RetentionStatus::Unconfigured.as_str(), "unconfigured");
    assert_eq!(RetentionStatus::Executed.as_str(), "executed");
}

// --- 2. Pre-creation pass (D-R1) ----------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn pre_creation_pass_creates_partitions_ahead_of_the_frontier() {
    let Some(url) = require_db("pre_creation_pass_creates_partitions_ahead_of_the_frontier")
    else {
        return;
    };
    let _db = db_lock();
    let admin_store = FlexCoreStore::connect(&url).expect("admin store");
    admin_store.run_migrations().expect("migrations");
    ensure_test_roles(admin_store.pool());
    let store = maintenance_store(&url);

    let now = SystemClock.now_event_time().timestamp();
    let current = partition_name(now.year(), u8::from(now.month()));
    let (next_year, next_month) = month_add(now.year(), u8::from(now.month()), 1);
    let next = partition_name(next_year, next_month);

    let created = pre_creation_pass(&store, 2).expect("pre-creation pass");
    // The pass names exactly what it created; both frontier months exist
    // afterwards with the D-P2 bounds.
    let names = attached_names(&store);
    for expected in [current, next] {
        assert!(
            names.iter().any(|(name, _)| *name == expected),
            "{expected} must exist after the pass (created list was {created:?})"
        );
        let (_, bounds) = names
            .iter()
            .find(|(name, _)| *name == expected)
            .expect("looked up above");
        assert!(
            bounds.is_some(),
            "{expected} must carry parseable D-P2 bounds"
        );
    }
}

// --- 3. Retention pass statuses (D-R2/D-R4) ------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn no_retention_configured_reports_and_deletes_nothing() {
    let Some(url) = require_db("no_retention_configured_reports_and_deletes_nothing") else {
        return;
    };
    let _db = db_lock();
    let admin_store = FlexCoreStore::connect(&url).expect("admin store");
    admin_store.run_migrations().expect("migrations");
    ensure_test_roles(admin_store.pool());
    let store = maintenance_store(&url);

    let before = attached_names(&store);
    let settings = MaintenanceSettings::from_config(2, None, false);
    let report = retention_pass(&store, &settings);
    assert_eq!(report.status, RetentionStatus::Unconfigured);
    assert!(report.detached.is_empty());
    // The D-R4 visibility: the note states the unconfigured state and that
    // no deletion occurs — never silent.
    assert!(report.note.contains("no retention is configured"));
    assert!(report.note.contains("no deletion occurs"));

    // Nothing was removed (a concurrent pre-creation may have added
    // partitions; nothing may disappear).
    let after = attached_names(&store);
    for (name, _) in &before {
        assert!(
            after.iter().any(|(n, _)| n == name),
            "the unconfigured pass must not remove {name}"
        );
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn configured_but_unapproved_retention_deletes_nothing() {
    let Some(url) = require_db("configured_but_unapproved_retention_deletes_nothing") else {
        return;
    };
    let _db = db_lock();
    let admin_store = FlexCoreStore::connect(&url).expect("admin store");
    admin_store.run_migrations().expect("migrations");
    ensure_test_roles(admin_store.pool());
    let store = maintenance_store(&url);

    let before = attached_names(&store);
    let settings = MaintenanceSettings::from_config(2, Some(2), false);
    let report = retention_pass(&store, &settings);
    assert_eq!(report.status, RetentionStatus::NotApproved);
    assert!(report.detached.is_empty());
    assert!(
        report.note.contains("not flagged approved"),
        "the D-R2 gate must be visible: {}",
        report.note
    );

    let after = attached_names(&store);
    for (name, _) in &before {
        assert!(
            after.iter().any(|(n, _)| n == name),
            "an unapproved pass must not remove {name}"
        );
    }
}

// --- 4. Approved pass: out-of-window-only detach + audit (D-R2/D-R3/D-R5) ------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn approved_pass_detaches_only_out_of_window_partitions_with_audit() {
    let Some(url) = require_db("approved_pass_detaches_only_out_of_window_partitions_with_audit")
    else {
        return;
    };
    let _db = db_lock();
    let admin_store = FlexCoreStore::connect(&url).expect("admin store");
    admin_store.run_migrations().expect("migrations");
    ensure_test_roles(admin_store.pool());
    let admin_pool = admin_store.pool();
    let store = maintenance_store(&url);

    // The test's OWN old month (no other suite uses 2019-05): entirely
    // outside any plausible retention window.
    {
        let unit = store.begin(None).expect("unit of work");
        let shared = Arc::new(unit);
        flexcore_postgres_adapter::PgUnitOfWork::create_telemetry_partition(&shared, 2019, 5)
            .expect("create 2019-05");
        Arc::try_unwrap(shared)
            .expect("unique")
            .commit()
            .expect("commit");
    }

    let now = SystemClock.now_event_time().timestamp();
    let cutoff = retention_cutoff(now.year(), u8::from(now.month()), 2).expect("cutoff");
    let before = attached_names(&store);
    let before_names: Vec<String> = before.iter().map(|(name, _)| name.clone()).collect();
    assert!(
        before_names.contains(&"telemetry_y2019m05".to_string()),
        "test precondition: the test's old partition is attached"
    );

    let settings = MaintenanceSettings::from_config(2, Some(2), true);
    let report = retention_pass(&store, &settings);
    assert_eq!(report.status, RetentionStatus::Executed, "{}", report.note);
    assert!(
        report.detached.contains(&"telemetry_y2019m05".to_string()),
        "the out-of-window partition must be detached: {:?}",
        report.detached
    );
    // Every detached partition was entirely outside the window (D-R3):
    // its captured upper bound is at or before the cutoff.
    for name in &report.detached {
        let bounds = before
            .iter()
            .find(|(n, _)| n == name)
            .and_then(|(_, bounds)| *bounds)
            .unwrap_or_else(|| panic!("{name} must have had captured bounds"));
        assert!(
            bounds.1 <= cutoff.unix_timestamp(),
            "{name} must lie entirely outside the retention window"
        );
    }
    // The current month's partition (in-flight) is never detached.
    let current = partition_name(now.year(), u8::from(now.month()));
    assert!(
        !report.detached.contains(&current),
        "the in-flight partition must never be detached"
    );

    // One audit record per action (D-R5), written in the same transaction
    // as the detach.
    assert!(
        audit_count_for(admin_pool, "telemetry.partition.detach", "telemetry_y2019m05") >= 1,
        "the detach must have an audit record"
    );
    // The partition is no longer attached but still exists (drop is the
    // second approved step).
    let after_names: Vec<String> = attached_names(&store)
        .into_iter()
        .map(|(name, _)| name)
        .collect();
    assert!(!after_names.contains(&"telemetry_y2019m05".to_string()));
    assert!(relation_exists(admin_pool, "telemetry_y2019m05"));

    // Shared-database courtesy: re-attach every partition this pass
    // detached that this test did not create (the other suites' months).
    for name in &report.detached {
        if name == "telemetry_y2019m05" {
            continue;
        }
        if let Some((_, Some(bounds))) = before.iter().find(|(n, _)| n == name) {
            reattach(admin_pool, name, *bounds);
        }
    }
}

// --- 5. The second approved step: drop (D-R2) -----------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn drop_step_refuses_attached_partitions_and_drops_detached_ones() {
    let Some(url) = require_db("drop_step_refuses_attached_partitions_and_drops_detached_ones")
    else {
        return;
    };
    let _db = db_lock();
    let admin_store = FlexCoreStore::connect(&url).expect("admin store");
    admin_store.run_migrations().expect("migrations");
    ensure_test_roles(admin_store.pool());
    let admin_pool = admin_store.pool();
    let store = maintenance_store(&url);

    // The drop test owns 2019-07 exclusively (the approved-pass test owns
    // 2019-05; no suite shares these months).

    // An ATTACHED partition: drop is refused (detach first, D-R2), no
    // audit record is written, the table remains.
    {
        let unit = store.begin(None).expect("unit of work");
        let shared = Arc::new(unit);
        flexcore_postgres_adapter::PgUnitOfWork::create_telemetry_partition(&shared, 2019, 7)
            .expect("create 2019-07");
        Arc::try_unwrap(shared).expect("unique").commit().expect("commit");
    }
    let before_drop =
        audit_count_for(admin_pool, "telemetry.partition.drop", "telemetry_y2019m07");
    let refused = flexcore_worker::maintenance::drop_step(
        &store,
        &["telemetry_y2019m07".to_string()],
        "integration-test:drop",
        "FLEXCORE_RETENTION_NORMALIZED_TELEMETRY_MONTHS",
    );
    assert!(refused.is_err(), "an attached partition must be refused");
    assert_eq!(
        audit_count_for(admin_pool, "telemetry.partition.drop", "telemetry_y2019m07"),
        before_drop,
        "a refused drop writes no audit record"
    );
    assert!(relation_exists(admin_pool, "telemetry_y2019m07"));

    // Detach 2019-07 (entirely outside any retention window vs the wall
    // clock), then the second approved step drops it with its audit record.
    {
        let now = SystemClock.now_event_time().timestamp();
        let unit = store.begin(None).expect("unit of work");
        use flexcore_postgres_adapter::partition_maintenance::PartitionAuditContext;
        let audit = PartitionAuditContext {
            audit_id: fresh_uuid(0x71),
            correlation_id: fresh_uuid(0x72),
            actor: "integration-test:drop".to_string(),
            config_source: "FLEXCORE_RETENTION_NORMALIZED_TELEMETRY_MONTHS".to_string(),
        };
        unit.detach_telemetry_partition("telemetry_y2019m07", now, &audit)
            .expect("detach for the drop test");
        unit.commit().expect("commit");
    }
    let dropped = flexcore_worker::maintenance::drop_step(
        &store,
        &["telemetry_y2019m07".to_string()],
        "integration-test:drop",
        "FLEXCORE_RETENTION_NORMALIZED_TELEMETRY_MONTHS",
    )
    .expect("drop of the detached partition");
    assert_eq!(dropped, vec!["telemetry_y2019m07".to_string()]);
    assert!(
        !relation_exists(admin_pool, "telemetry_y2019m07"),
        "the dropped table must be gone"
    );
    assert!(
        audit_count_for(admin_pool, "telemetry.partition.drop", "telemetry_y2019m07") >= 1,
        "the drop must have an audit record"
    );
}

fn fresh_uuid(tag: u8) -> flexcore_contracts::ids::FlexUuid {
    let mut bytes = [0u8; 16];
    bytes[0] = tag;
    bytes[1] = 0x6B;
    bytes[2..10].copy_from_slice(&run_key().to_be_bytes());
    flexcore_contracts::ids::FlexUuid::from_bytes(bytes)
}
