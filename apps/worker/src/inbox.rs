// P1.19 — The durable PostgreSQL `InboxStore` (P1.16 step 1) and the
// recorded single-home decision.
//
// # Single-home decision (recorded; supersedes the P1.16 interim plan)
//
// `InboxStore` (the trait) is defined ONCE, in
// `crates/adapters/nats/src/inbox.rs` (P1.16 decision 2). The P1.16 header
// planned the durable implementation for `crates/adapters/postgres`; this
// subtask's edit scope is `apps/worker/**` only, so the durable
// implementation lands HERE (`flexcore_worker::inbox::PgInboxStore`)
// instead. Consequences, stated explicitly:
//
// 1. This is an interim single home, not a second definition: there is
//    exactly one durable implementation of the trait in the workspace, and
//    it lives in the composition root that wires it.
// 2. The inbox TABLE is likewise created here (`ensure_table`,
//    `CREATE TABLE IF NOT EXISTS`) because the postgres adapter's
//    migration set is outside this subtask's edit scope. The P1.12/P1.16
//    inbox schema obligations are honoured: per-consumer key, UNIQUE
//    constraint, consumed-at timestamp, consumer identity.
// 3. When the store is promoted into `flexcore-postgres-adapter`
//    (P1.30 boundary audit item), the table DDL moves into the adapter's
//    migration set and THIS MODULE IS DELETED — one home, one definition
//    (P1.10 single-home rule).
//
// # Sync/async bridge and the transactional obligation
//
// The trait is sync (house rule); this implementation follows the
// `FlexCoreStore` pattern exactly: a private tokio runtime per store, sync
// methods block on it. Callers MUST invoke the methods from a blocking
// context — inside the worker's loops that means `block_in_place` (see
// `runner`); calling them directly from a runtime worker thread panics,
// the same contract the postgres adapter records.
//
// The "record inbox + state change in ONE transaction" obligation
// (`nats/src/inbox.rs` module header) is binding on HANDLERS. The Phase 1
// consumer handler (`runner::inbox_recording_handler`) has no other state
// change yet: its side effect IS the inbox record, executed as the
// single-statement transaction here. When real consumers with business
// side effects arrive, the handler opens the transaction (via the
// composition root's storage) and calls `record` inside it — this store's
// `record` is then expected to be called through the unit-of-work path,
// which is exactly what the promotion into the postgres adapter enables.

use std::sync::Arc;
use std::time::Duration;

use flexcore_nats_adapter::inbox::{InboxStore, InboxStoreError, RecordOutcome};
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use tokio::runtime::Runtime;

/// The worker-owned inbox table. The UNIQUE constraint is the primary key
/// on `(consumer_name, idempotency_key)` (P1.16 step 1: "unique
/// constraint, consumed-at timestamp, and consumer identity").
const SQL_ENSURE_TABLE: &str = "CREATE TABLE IF NOT EXISTS flexcore_worker_inbox (\
     consumer_name text NOT NULL, \
     idempotency_key text NOT NULL, \
     consumed_at timestamptz NOT NULL DEFAULT now(), \
     consumer_identity text NOT NULL DEFAULT 'flexcore-worker', \
     PRIMARY KEY (consumer_name, idempotency_key))";

const SQL_IS_RECORDED: &str = "SELECT EXISTS(\
     SELECT 1 FROM flexcore_worker_inbox \
     WHERE consumer_name = $1 AND idempotency_key = $2)";

const SQL_RECORD: &str = "INSERT INTO flexcore_worker_inbox \
     (consumer_name, idempotency_key) VALUES ($1, $2) \
     ON CONFLICT (consumer_name, idempotency_key) DO NOTHING";

/// How long a backend call may take before the private runtime is
/// considered wedged; bounded so a stuck database cannot hang a loop
/// forever without the caller's own backoff noticing.
const CALL_TIMEOUT: Duration = Duration::from_secs(30);

/// Durable inbox store backed by PostgreSQL. One private runtime, like
/// `FlexCoreStore` (see the module header for the blocking-context
/// contract). Clones share the pool and runtime.
#[derive(Clone)]
pub struct PgInboxStore {
    pool: PgPool,
    runtime: Arc<Runtime>,
}

impl PgInboxStore {
    /// Builds the store on an existing pool (the worker shares the
    /// outbox store's pool; the connections are the same database).
    pub fn new(pool: PgPool) -> Result<Self, InboxStoreError> {
        let runtime = Runtime::new().map_err(|_| InboxStoreError::Backend(
            "the inbox store's async runtime could not be started".to_string(),
        ))?;
        Ok(Self {
            pool,
            runtime: Arc::new(runtime),
        })
    }

    /// Connects a dedicated pool. For tests and tooling; the worker wires
    /// [`new`] with the shared pool instead.
    pub fn connect(url: &str) -> Result<Self, InboxStoreError> {
        let runtime = Runtime::new().map_err(|_| InboxStoreError::Backend(
            "the inbox store's async runtime could not be started".to_string(),
        ))?;
        let pool = runtime
            .block_on(async {
                PgPoolOptions::new()
                    .max_connections(5)
                    .connect(url)
                    .await
            })
            .map_err(|_| InboxStoreError::Backend(
                "the inbox store's database could not be reached with the configured URL"
                    .to_string(),
            ))?;
        Ok(Self {
            pool,
            runtime: Arc::new(runtime),
        })
    }

    /// Creates the inbox table if absent (idempotent; run at startup).
    /// Interim measure — see the module header for the promotion path.
    pub fn ensure_table(&self) -> Result<(), InboxStoreError> {
        let pool = self.pool.clone();
        self.block(async move {
            sqlx::query(SQL_ENSURE_TABLE).execute(&pool).await
        })
        .map(|_| ())
    }

    /// Runs one async database operation on the private runtime, with a
    /// call timeout and opaque error mapping (no SQL text in errors).
    fn block<T>(
        &self,
        fut: impl std::future::Future<Output = sqlx::Result<T>>,
    ) -> Result<T, InboxStoreError> {
        self.runtime
            .block_on(async { tokio::time::timeout(CALL_TIMEOUT, fut).await })
            .map_err(|_| {
                InboxStoreError::Backend(
                    "the inbox store's database call timed out".to_string(),
                )
            })?
            .map_err(|_| {
                InboxStoreError::Backend(
                    "the inbox store's database call failed".to_string(),
                )
            })
    }
}

impl InboxStore for PgInboxStore {
    fn is_recorded(
        &self,
        consumer_name: &str,
        idempotency_key: &str,
    ) -> Result<bool, InboxStoreError> {
        let pool = self.pool.clone();
        let consumer = consumer_name.to_string();
        let key = idempotency_key.to_string();
        self.block(async move {
            let row: (bool,) = sqlx::query_as(SQL_IS_RECORDED)
                .bind(consumer)
                .bind(key)
                .fetch_one(&pool)
                .await?;
            Ok(row.0)
        })
    }

    fn record(
        &self,
        consumer_name: &str,
        idempotency_key: &str,
    ) -> Result<RecordOutcome, InboxStoreError> {
        let pool = self.pool.clone();
        let consumer = consumer_name.to_string();
        let key = idempotency_key.to_string();
        let inserted = self.block(async move {
            let result = sqlx::query(SQL_RECORD)
                .bind(consumer)
                .bind(key)
                .execute(&pool)
                .await?;
            Ok(result.rows_affected() == 1)
        })?;
        if inserted {
            Ok(RecordOutcome::Inserted)
        } else {
            // The unique constraint refused the duplicate: the caller MUST
            // treat this as "do not execute the side effect" (trait
            // obligation; the concurrent-duplicate rollback path).
            Ok(RecordOutcome::AlreadyRecorded)
        }
    }
}
