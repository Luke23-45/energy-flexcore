// P1.13 — FlexCoreStore: the PostgreSQL store (pool + runtime), the unit-of
//         work factory, the embedded migration runner, the publisher-side
//         outbox port, and the self-scoping audit sink.
//
// Role model (P1.12 §3/§6, [R25]): the runtime connects as `flexcore_app`
// — NEVER as the table owner. `flexcore_migrator` owns the tables and runs
// only migrations. `FORCE ROW LEVEL SECURITY` on the five tenant-owned
// tables makes even owner sessions subject to the policies, but the
// application must still never hold owner credentials.
//
// Sync/async bridge (recorded decision): the port traits are sync (P1.07
// recorded rule); this store owns a private Tokio runtime and every port
// method blocks on it. Callers inside an async service must invoke the
// ports from a blocking context (e.g. `tokio::task::spawn_blocking`) —
// blocking inside an async execution context would panic.

use crate::error::{codes, map_audit_rejection, map_backend};
use crate::id_generator::PostgresIdGenerator;
use crate::outbox;
use crate::unit_of_work::PgUnitOfWork;
use crate::uuid_map::uuid_text;
use flexcore_audit::record::AuditRecord;
use flexcore_audit::sink::AuditSink;
use flexcore_domain::tenant_context::TenantContext;
use flexcore_ports::backend_error::BackendError;
use flexcore_ports::event_outbox::{EventOutbox, EventOutboxError, OutboxWrite, PendingOutboxEntry};
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use std::sync::Arc;
use tokio::runtime::Runtime;

/// The PostgreSQL store: a connection pool, a private runtime for the sync
/// port bridge, and the embedded migrations.
#[derive(Clone)]
pub struct FlexCoreStore {
    pool: PgPool,
    runtime: Arc<Runtime>,
}

impl FlexCoreStore {
    /// Connects a pool to the database at `url`. The connecting role is a
    /// deployment decision: the runtime role is `flexcore_app` (P1.12 §3.1);
    /// migrations need the owner role (`flexcore_migrator`) or a bootstrap
    /// administrator with `CREATEROLE`.
    pub fn connect(url: &str) -> Result<Self, BackendError> {
        let runtime = Arc::new(Runtime::new().map_err(|_| {
            BackendError::new(
                codes::RUNTIME.to_string(),
                "the adapter's async runtime could not be started".to_string(),
            )
        })?);
        let pool = runtime
            .block_on(async {
                PgPoolOptions::new()
                    .max_connections(5)
                    .connect(url)
                    .await
            })
            .map_err(|_| {
                BackendError::new(
                    codes::UNAVAILABLE.to_string(),
                    "the storage backend could not be reached with the configured URL".to_string(),
                )
            })?;
        Ok(Self { pool, runtime })
    }

    /// The underlying pool, for composition-root wiring and tests that run
    /// raw administrative SQL. Application code uses the ports.
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// Applies the embedded migrations (P1.13 `migrations/`, implementing
    /// the P1.12 design). Idempotent: already-applied versions are skipped.
    /// Down migrations are a dev/CI rollback concern (P1.12 §7) and are
    /// exercised by the integration tests, not by this runner.
    pub fn run_migrations(&self) -> Result<(), BackendError> {
        static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("migrations/");
        let pool = self.pool.clone();
        self.runtime.block_on(async move {
            MIGRATOR.run(&pool).await.map_err(migration_failure)
        })
    }

    /// Opens one unit of work: the native transaction all port calls of a
    /// use-case invocation run inside (P1.07 `EventOutbox` binding rule;
    /// P1.10 adapter contract). When `ctx` is given, the transaction-scoped
    /// RLS tenant context (`flexcore.tenant_id`) is set before the first
    /// statement; without it, tenant-owned tables fail closed (P1.12 §5.1).
    /// Dropping the returned unit of work without `commit`/`rollback` rolls
    /// the native transaction back (P1.10 adapter contract rule 4).
    pub fn begin(&self, ctx: Option<&TenantContext>) -> Result<PgUnitOfWork, BackendError> {
        let runtime = Arc::clone(&self.runtime);
        let pool = self.pool.clone();
        let tenant = ctx.map(|c| c.tenant_id());
        let transaction = runtime.block_on(async move {
            let mut tx = pool.begin().await.map_err(map_backend)?;
            if let Some(tenant) = &tenant {
                sqlx::query("SELECT set_config('flexcore.tenant_id', $1, true)")
                    .bind(uuid_text(tenant.uuid()))
                    .execute(&mut *tx)
                    .await
                    .map_err(map_backend)?;
            }
            Ok(tx)
        })?;
        Ok(PgUnitOfWork::assemble(transaction, runtime))
    }

    /// The ID generator sharing this store's pool and runtime (D-S1
    /// version policy: application identifiers are PostgreSQL `uuidv7()`,
    /// like every primary-key default).
    pub fn id_generator(&self) -> PostgresIdGenerator {
        PostgresIdGenerator::new(self.pool.clone(), Arc::clone(&self.runtime))
    }
}

/// Maps a migration failure to an opaque backend error. The message
/// describes the failure class only; migration SQL never travels into a
/// port error.
fn migration_failure(err: sqlx::migrate::MigrateError) -> BackendError {
    let message = match &err {
        sqlx::migrate::MigrateError::Source(_) => {
            "the embedded migrations could not be read".to_string()
        }
        sqlx::migrate::MigrateError::VersionMissing(version) => {
            format!("applied migration version {version} is missing from the deployed binary")
        }
        sqlx::migrate::MigrateError::VersionMismatch(_) => {
            "an applied migration no longer matches the deployed binary".to_string()
        }
        sqlx::migrate::MigrateError::InvalidMigrationVersion(_) => {
            "a migration file carries an unusable version number".to_string()
        }
        sqlx::migrate::MigrateError::NotInTransaction => {
            "a migration required a transaction but none was open".to_string()
        }
        sqlx::migrate::MigrateError::Execute(inner) => map_backend(inner).message().to_string(),
        _ => "migration execution failed on the storage backend".to_string(),
    };
    BackendError::new(codes::MIGRATION.to_string(), message)
}

/// The store-level `append` guard: the transactional outbox write must
/// execute inside the caller's unit of work (P1.07 `EventOutbox` port
/// obligation; the P1.10 `TransactionalWrite` adapter contract). Without a
/// unit of work there is no transaction to share with the state change, so
/// the call fails closed instead of opening a dual-write hole.
pub(crate) fn outbox_append_requires_unit_of_work() -> EventOutboxError {
    EventOutboxError::Backend(BackendError::new(
        codes::OUTBOX_REQUIRES_UOW.to_string(),
        "the transactional outbox write must execute inside the caller's unit of work; \
         open one with FlexCoreStore::begin and append through it"
            .to_string(),
    ))
}

impl EventOutbox for FlexCoreStore {
    /// Refused: see [`outbox_append_requires_unit_of_work`]. Appending
    /// happens through [`PgUnitOfWork`] so the state change and the outbox
    /// row share one native transaction.
    fn append(&self, _write: OutboxWrite) -> Result<(), EventOutboxError> {
        Err(outbox_append_requires_unit_of_work())
    }

    /// Up to `limit` pending entries in insertion order, for the outbox
    /// publisher (P1.15). Runs on the pool (its own autocommit read).
    fn pending(&self, limit: u32) -> Result<Vec<PendingOutboxEntry>, EventOutboxError> {
        let pool = self.pool.clone();
        self.runtime
            .block_on(async move { outbox::pending_entries(&pool, limit).await })
    }

    /// Records one publish attempt (delivery state unchanged).
    fn record_attempt(&self, idempotency_key: &str) -> Result<(), EventOutboxError> {
        let pool = self.pool.clone();
        let key = idempotency_key.to_string();
        self.runtime
            .block_on(async move { outbox::record_attempt(&pool, &key).await })
    }

    /// Marks the entry delivered — only after broker acknowledgement.
    fn mark_delivered(&self, idempotency_key: &str) -> Result<(), EventOutboxError> {
        let pool = self.pool.clone();
        let key = idempotency_key.to_string();
        self.runtime
            .block_on(async move { outbox::mark_delivered(&pool, &key).await })
    }

    /// Marks the entry failed after the retry policy was exhausted.
    fn mark_failed(&self, idempotency_key: &str) -> Result<(), EventOutboxError> {
        let pool = self.pool.clone();
        let key = idempotency_key.to_string();
        self.runtime
            .block_on(async move { outbox::mark_failed(&pool, &key).await })
    }
}

impl AuditSink for FlexCoreStore {
    /// Appends one audit record in its own short transaction. The RLS
    /// tenant context is derived from the record's own tenant attribution
    /// (the record was built from authenticated claims upstream, P0.16
    /// rule 5): a platform-level record (`tenant_id IS NULL`) is therefore
    /// only appendable through the owner/ops path, matching P1.12 §5.2.
    /// For writes that belong to a use-case unit of work, use the sink on
    /// [`PgUnitOfWork`] instead.
    fn append(&self, record: AuditRecord) -> Result<(), flexcore_audit::record::AuditError> {
        let pool = self.pool.clone();
        self.runtime.block_on(async move {
            let mut tx = pool.begin().await.map_err(map_audit_rejection)?;
            if let Some(tenant) = record.tenant_id {
                sqlx::query("SELECT set_config('flexcore.tenant_id', $1, true)")
                    .bind(uuid_text(tenant.uuid()))
                    .execute(&mut *tx)
                    .await
                    .map_err(map_audit_rejection)?;
            }
            crate::audit::insert_audit_record(&mut *tx, &record).await?;
            tx.commit().await.map_err(map_audit_rejection)?;
            Ok(())
        })
    }
}
