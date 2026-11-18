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
//
// P11.15 ADDITION — the keyed audit chain (the P11-E-1 DEFAULT): the
// deployment supplies the chain namespace key via
// [`FlexCoreStore::with_audit_chain_key`]; when configured, every audit
// append (both the unit-of-work sink and this store's self-scoping sink)
// advances the per-tenant keyed HMAC-SHA-256 chain
// (`flexcore_audit::chain`, migration 0019), and
// [`FlexCoreStore::verify_tenant_audit_chain`] recomputes a tenant's chain
// from the stored rows. THE KEY IS A DEPLOYMENT REQUIREMENT: key
// generation, distribution, rotation, and storage are human-gated
// deployment concerns (program conventions §6.6) — this store consumes a
// key it is handed and never generates, loads, or persists one. Without a
// configured key the chain columns stay NULL and rows are verifiable only
// through the TEST-GRADE FNV-1a edge chain (LR-203), never a release
// signature.
//
// P11.16 ADDITION — the SQLx pool credential-refresh DESIGN (WRITTEN, NOT
// DEPLOYED — nothing here rotates anything today):
//
//   The pool is built ONCE in `connect` from a static connection string;
//   with dynamic short-lived credentials (the recorded Vault DEFAULT,
//   P11.16 decision record: Postgres credentials with ~1h TTL [S-74]),
//   that pool eventually holds dead credentials and connections are
//   dropped by the server at lease expiry. The recorded refresh design,
//   for the wave that implements it, anchors at THIS seam:
//
//   1. The `pool` field becomes an atomically swappable container (e.g.
//      `Arc<RwLock<PgPool>>` or an arc-swap cell) — the single mechanical
//      refactor point, because every port call already reaches the pool
//      through this struct.
//   2. A supervision task (on this store's runtime) holds the credential
//      lease: at a configured fraction of the TTL it acquires the NEW
//      credentials, builds a SECOND pool with the same options, and only
//      on a successful connect swaps the container; the old pool is
//      closed with a grace overlap so in-flight units of work drain
//      (their transactions live on connections already checked out, which
//      closing the old pool after the grace period does not disturb).
//   3. Lease revocation or renewal failure fails LOUD (the supervisor
//      surfaces the error to the composition root's health/observability
//      surface) — a silently expiring credential pool is a hidden outage.
//   4. Never baked into this adapter: the Vault client, lease semantics,
//      and endpoint configuration [S-74][S-75] are infrastructure the
//      composition root injects; the adapter stays credential-source
//      agnostic (a `Fn() -> Future<Result<String, _>>` credential-provider
//      port at this seam is the recorded shape).
//
//   Nothing above is implemented here — this block is the design record
//   the implementing wave starts from; no behavior claims are made.

use crate::audit;
// P11.15 re-exports: the chain-verification report types live in the
// crate-internal `audit` module (its SQL statements are crate-internal by
// the P4.36 pattern), but the store-level verification method is public —
// the types are re-exported here so the public interface stays nameable.
pub use crate::audit::{AuditChainVerificationOutcome, TenantChainVerification};
use crate::error::{codes, map_audit_rejection, map_backend};
use crate::id_generator::PostgresIdGenerator;
use crate::outbox;
use crate::telemetry_reader::PgTelemetryReader;
use crate::unit_of_work::PgUnitOfWork;
use crate::uuid_map::uuid_text;
use flexcore_audit::chain::ChainKey;
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
    /// The optional deployment-supplied audit-chain namespace key. `None`
    /// is the honest un-keyed state: chain columns stay NULL (P11.15).
    audit_chain_key: Option<Arc<ChainKey>>,
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
        Ok(Self {
            pool,
            runtime,
            audit_chain_key: None,
        })
    }

    /// Configures the deployment-supplied audit-chain namespace key
    /// (P11.15, the P11-E-1 DEFAULT construction). KEY MANAGEMENT IS THE
    /// DEPLOYMENT REQUIREMENT gate: the caller hands over key material it
    /// obtained through the deployment's reviewed key-management process —
    /// this store never generates or persists keys, and configuring one
    /// here does NOT mean the key-management plan has been signed off.
    /// Chain-keyed operation is valid only after that human sign-off
    /// (the P11.15 decision record; P11-E-1 stays OPEN).
    pub fn with_audit_chain_key(mut self, key: ChainKey) -> Self {
        self.audit_chain_key = Some(Arc::new(key));
        self
    }

    /// Whether a chain key is configured. Composition-root diagnostics
    /// only — `true` means keyed chain appends will be attempted, NOT that
    /// the deployment requirement has been met.
    pub fn audit_chain_configured(&self) -> bool {
        self.audit_chain_key.is_some()
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
        static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");
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
        let chain_key = self.audit_chain_key.clone();
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
        Ok(PgUnitOfWork::assemble(transaction, runtime, chain_key))
    }

    /// P11.15 — recomputes one tenant's keyed audit chain from the stored
    /// rows (migration 0019 columns) and returns the named report.
    /// WRITTEN, NEVER RUN: this is the per-tenant verification path the
    /// P11.15 decision record describes; running it against a live chain
    /// is a human-gated operation. The tenant context governs RLS (the
    /// transaction-scoped `flexcore.tenant_id` is set exactly as in
    /// [`FlexCoreStore::begin`]); a tenant with no chained rows verifies
    /// with zero records, and the unchained-row count is reported, never
    /// silently skipped.
    pub fn verify_tenant_audit_chain(
        &self,
        ctx: &TenantContext,
    ) -> Result<TenantChainVerification, BackendError> {
        let runtime = Arc::clone(&self.runtime);
        let pool = self.pool.clone();
        let chain_key = self.audit_chain_key.clone();
        let tenant = ctx.tenant_id();
        runtime.block_on(async move {
            let namespace_key = chain_key.ok_or_else(|| {
                BackendError::new(
                    codes::REJECTED.to_string(),
                    "no audit-chain key is configured; the keyed chain verification requires \
                     the deployment to supply one (a DEPLOYMENT REQUIREMENT, P11.15)"
                        .to_string(),
                )
            })?;
            let mut tx = pool.begin().await.map_err(map_backend)?;
            sqlx::query("SELECT set_config('flexcore.tenant_id', $1, true)")
                .bind(uuid_text(tenant.uuid()))
                .execute(&mut *tx)
                .await
                .map_err(map_backend)?;
            let verification = audit::verify_tenant_chain_rows(&mut tx, tenant, &namespace_key)
                .await
                .map_err(|rejection| {
                    BackendError::new(
                        codes::DECODE.to_string(),
                        rejection.to_string(),
                    )
                })?;
            tx.commit().await.map_err(map_backend)?;
            Ok(verification)
        })
    }

    /// The ID generator sharing this store's pool and runtime (D-S1
    /// version policy: application identifiers are PostgreSQL `uuidv7()`,
    /// like every primary-key default).
    pub fn id_generator(&self) -> PostgresIdGenerator {
        PostgresIdGenerator::new(self.pool.clone(), Arc::clone(&self.runtime))
    }

    /// The telemetry READ port (P2.12) over this store's pool: a
    /// pool-held [`PgTelemetryReader`] — each trait call opens its own
    /// tenant-scoped unit of work via [`FlexCoreStore::begin`] (see that
    /// module for why the reader is pool-held rather than bound to a
    /// request's unit of work).
    pub fn telemetry_reader(&self) -> PgTelemetryReader {
        PgTelemetryReader::new(self.clone())
    }
}

/// Maps a migration failure to an opaque backend error. The message
/// describes the failure class only; migration SQL never travels into a
/// port error.
fn migration_failure(err: sqlx::migrate::MigrateError) -> BackendError {
    // Matched by value so the `Execute` arm hands over the owned
    // `sqlx::Error` to `map_backend` (it takes the error by value).
    // Two arms from the pre-0.9 mapping are gone: `InvalidMigrationVersion`
    // and `NotInTransaction` do not exist in sqlx 0.9's `MigrateError`
    // (a bad version file fails resolution and surfaces as `Source`;
    // migrations always run inside a transaction, so there is no
    // not-in-transaction variant).
    let message = match err {
        sqlx::migrate::MigrateError::Source(_) => {
            "the embedded migrations could not be read".to_string()
        }
        sqlx::migrate::MigrateError::VersionMissing(version) => {
            format!("applied migration version {version} is missing from the deployed binary")
        }
        sqlx::migrate::MigrateError::VersionMismatch(_) => {
            "an applied migration no longer matches the deployed binary".to_string()
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
    ///
    /// P11.15: when the deployment configured a chain key
    /// ([`FlexCoreStore::with_audit_chain_key`]), the append also advances
    /// the record's TENANT-NAMESPACE keyed chain
    /// (`crate::audit::append_audit_record_with_chain`) inside the same
    /// transaction. Without a key the chain columns stay NULL — the honest
    /// un-keyed state (DEPLOYMENT REQUIREMENT, P11.15).
    fn append(&self, record: AuditRecord) -> Result<(), flexcore_audit::record::AuditError> {
        let pool = self.pool.clone();
        let chain_key = self.audit_chain_key.clone();
        self.runtime.block_on(async move {
            let mut tx = pool.begin().await.map_err(map_audit_rejection)?;
            if let Some(tenant) = record.tenant_id {
                sqlx::query("SELECT set_config('flexcore.tenant_id', $1, true)")
                    .bind(uuid_text(tenant.uuid()))
                    .execute(&mut *tx)
                    .await
                    .map_err(map_audit_rejection)?;
            }
            match &chain_key {
                Some(key) => {
                    let namespace = key.chain_key_for_namespace(record.tenant_id);
                    crate::audit::append_audit_record_with_chain(&mut tx, &record, &namespace)
                        .await?;
                }
                None => {
                    crate::audit::insert_audit_record(&mut *tx, &record, None).await?;
                }
            }
            tx.commit().await.map_err(map_audit_rejection)?;
            Ok(())
        })
    }
}
