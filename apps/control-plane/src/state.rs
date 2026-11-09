// P1.17 — Application state: the backend handle and the unit-of-work runner.
//
// CRITICAL recorded constraint (P1.13 adapter): the real repository ports
// are SYNCHRONOUS and TRANSACTION-BOUND — the PostgreSQL adapter implements
// ALL of the repository ports, the outbox, and the audit sink on
// `PgUnitOfWork`, a single native sqlx transaction. There are therefore NO
// shared long-lived repository objects to hold in state. Every request's
// port calls go through [`AppState::run_in_uow`]: the Postgres backend
// opens one unit of work per invocation on a blocking thread, coerces the
// ONE `Arc<PgUnitOfWork>` into the five port trait objects (all pointing at
// the SAME transaction), builds the use cases, runs the operation, and
// commits via `Arc::try_unwrap`. The Test backend runs directly against the
// flexcore-ports fakes (no transaction).
use std::sync::Arc;

use flexcore_application::payload_encoding::EventPayloadEncoder;
use flexcore_authz::policy::PolicySet;
use flexcore_domain::tenant_context::TenantContext;
use flexcore_ports::clock::Clock;
use flexcore_ports::event_outbox::EventOutbox;
use flexcore_ports::id_generator::IdGenerator;
use flexcore_ports::repository::{
    AssetRepository, PortfolioRepository, SiteRepository, TenantRepository,
};
use flexcore_storage::envelope_store::EnvelopeRecordStore;
use flexcore_storage::schedule_revision::ScheduleRevisionChainStore;
use flexcore_storage::schedule_store::ScheduleRecordStore;

use crate::auth::IdentityResolver;
use crate::error::ApiError;
use crate::metrics::MetricsRegistry;
use crate::probes::Readiness;

/// The wired backend: the real PostgreSQL adapter, or the ports fakes
/// (tests only). In both variants the repository/outbox ports are NOT held
/// here for request use — they arrive per request through the
/// [`UnitOfWork`] bundle; the Postgres variant keeps only the transaction
/// factory (`FlexCoreStore`) plus the two non-transaction-bound ports
/// (IDs, clock).
#[derive(Clone)]
pub enum Backend {
    /// The real adapter (composition root): the store is the unit-of-work
    /// factory; `ids` is the pool-backed UUIDv7 generator; `clock` is the
    /// wall clock (both are transaction-independent ports).
    Postgres {
        store: Arc<flexcore_postgres_adapter::FlexCoreStore>,
        ids: Arc<dyn IdGenerator>,
        clock: Arc<dyn Clock>,
    },
    /// The ports fakes (integration tests): the repositories and the
    /// outbox are long-lived in-memory fakes; every operation runs
    /// directly, no transaction (the fakes journal the intended ordering).
    /// P3.28: the Phase 3 record stores (envelopes, §12.6 optimization
    /// runs, schedule revisions) join the bundle as the in-crate fakes
    /// ([`crate::phase3_fakes`]).
    Test {
        tenants: Arc<dyn TenantRepository>,
        portfolios: Arc<dyn PortfolioRepository>,
        sites: Arc<dyn SiteRepository>,
        assets: Arc<dyn AssetRepository>,
        outbox: Arc<dyn EventOutbox>,
        ids: Arc<dyn IdGenerator>,
        clock: Arc<dyn Clock>,
        schedule_records: Arc<dyn ScheduleRecordStore>,
        schedule_revisions: Arc<dyn ScheduleRevisionChainStore>,
        envelope_records: Arc<dyn EnvelopeRecordStore>,
    },
}

/// The ports one unit of work exposes to a use-case invocation.
///
/// For the Postgres backend every transaction-bound `Arc<dyn ...>` coerces
/// from the SAME allocation (one `PgUnitOfWork`), so the repository writes,
/// the outbox append, and any read all execute inside ONE native
/// transaction (the §6.3 step-1 rule holds by construction). For the Test
/// backend each field is the shared in-memory fake.
///
/// P3.28: the Phase 3 record stores join the bundle —
/// [`ScheduleRecordStore`] (the §12.6 optimization-run records, P3.21),
/// [`ScheduleRevisionChainStore`] (the schedule revision chains, P3.22),
/// and [`EnvelopeRecordStore`] (the flexibility-envelope records, P3.28
/// migration 0015). In the Postgres backend all three coerce from the same
/// `PgUnitOfWork` allocation, so their inserts and the paired outbox rows
/// are one native transaction (§15.5).
pub struct UnitOfWork {
    pub tenants: Arc<dyn TenantRepository>,
    pub portfolios: Arc<dyn PortfolioRepository>,
    pub sites: Arc<dyn SiteRepository>,
    pub assets: Arc<dyn AssetRepository>,
    pub outbox: Arc<dyn EventOutbox>,
    pub ids: Arc<dyn IdGenerator>,
    pub clock: Arc<dyn Clock>,
    /// The §12.6 schedule-result (optimization-run) record store (P3.21;
    /// P3.28 listing methods).
    pub schedule_records: Arc<dyn ScheduleRecordStore>,
    /// The schedule identity/revision chain store (P3.22; P3.28 listing).
    pub schedule_revisions: Arc<dyn ScheduleRevisionChainStore>,
    /// The flexibility-envelope record store (P3.28, migration 0015).
    pub envelope_records: Arc<dyn EnvelopeRecordStore>,
}

impl UnitOfWork {
    /// The event payload encoder for use-case composition (always the
    /// serde_json rendering; built per invocation, it is a unit struct).
    pub fn event_encoder(&self) -> Arc<dyn EventPayloadEncoder> {
        Arc::new(crate::encoding::JsonEventPayloadEncoder)
    }
}

/// The per-request application state.
#[derive(Clone)]
pub struct AppState {
    pub backend: Backend,
    pub policies: Arc<PolicySet>,
    pub identity: Arc<dyn IdentityResolver>,
    pub metrics: Arc<MetricsRegistry>,
    pub readiness: Arc<Readiness>,
    /// The telemetry READ port (P2.12/P2.17/P2.18 routes). Deliberately NOT
    /// part of the [`UnitOfWork`] bundle: the Postgres adapter does not
    /// implement the port yet (recorded P2.06-adjacent gap — the
    /// implementation must land ON `PgUnitOfWork` so the read runs inside
    /// the request transaction with RLS applied). Until then the value is
    /// `None` in the Postgres composition root and the telemetry routes
    /// fail closed (infrastructure error); the Test backend wires the
    /// in-crate fake.
    pub telemetry: Option<Arc<dyn flexcore_ports::telemetry_read::TelemetryReader>>,
    /// The CURRENT-STATE use-case freshness limit in milliseconds (P2.18
    /// step 3, configuration — `FLEXCORE_CURRENT_STATE_FRESHNESS_MS`,
    /// default 300 000 ms; the historical view defines its own limit). A
    /// value whose event-time age exceeds the limit is presented with the
    /// STALE flag, never as GOOD (P2.18: the last known value is shown
    /// WITH the STALE flag).
    pub current_state_freshness_millis: i64,
}

impl AppState {
    /// Composes the state from a backend, the authz policy set, the
    /// identity resolver, and the readiness bundle. The metrics registry
    /// is created here (one per state). `telemetry` is the read port (see
    /// the field docs for the recorded wiring status);
    /// `current_state_freshness_millis` is the P2.18 current-state
    /// freshness limit (configuration, must be positive).
    pub fn new(
        backend: Backend,
        policies: PolicySet,
        identity: Arc<dyn IdentityResolver>,
        readiness: Readiness,
        telemetry: Option<Arc<dyn flexcore_ports::telemetry_read::TelemetryReader>>,
        current_state_freshness_millis: i64,
    ) -> Self {
        assert!(
            current_state_freshness_millis > 0,
            "the current-state freshness limit must be a positive millisecond value \
             (validated at configuration load)"
        );
        Self {
            backend,
            policies: Arc::new(policies),
            identity,
            metrics: Arc::new(MetricsRegistry::new()),
            readiness: Arc::new(readiness),
            telemetry,
            current_state_freshness_millis,
        }
    }

    /// Test composition: the ports fakes variant plus explicit policies,
    /// identity, and readiness. Used by `tests/api.rs` and nothing else.
    /// P3.28: the Phase 3 record stores default to fresh in-crate fakes
    /// ([`crate::phase3_fakes`]) — use [`Self::for_tests_with_phase3`] when
    /// a test needs to inspect or inject them (or the outbox).
    #[allow(clippy::too_many_arguments)]
    pub fn for_tests(
        tenants: Arc<dyn TenantRepository>,
        portfolios: Arc<dyn PortfolioRepository>,
        sites: Arc<dyn SiteRepository>,
        assets: Arc<dyn AssetRepository>,
        outbox: Arc<dyn EventOutbox>,
        ids: Arc<dyn IdGenerator>,
        clock: Arc<dyn Clock>,
        policies: PolicySet,
        identity: Arc<dyn IdentityResolver>,
        readiness: Readiness,
        telemetry: Option<Arc<dyn flexcore_ports::telemetry_read::TelemetryReader>>,
        current_state_freshness_millis: i64,
    ) -> Self {
        Self::for_tests_with_phase3(
            tenants,
            portfolios,
            sites,
            assets,
            outbox,
            ids,
            clock,
            policies,
            identity,
            readiness,
            telemetry,
            current_state_freshness_millis,
            Arc::new(crate::phase3_fakes::FakeScheduleRecordStore::new()),
            Arc::new(crate::phase3_fakes::FakeScheduleRevisionStore::new()),
            Arc::new(crate::phase3_fakes::FakeEnvelopeRecordStore::new()),
        )
    }

    /// Test composition with explicit Phase 3 record stores (P3.28): the
    /// same bundle as [`Self::for_tests`] with the schedule-record,
    /// schedule-revision, and envelope-record fakes (or failing
    /// injections) supplied by the test. Used by `tests/api_phase3.rs`.
    #[allow(clippy::too_many_arguments)]
    pub fn for_tests_with_phase3(
        tenants: Arc<dyn TenantRepository>,
        portfolios: Arc<dyn PortfolioRepository>,
        sites: Arc<dyn SiteRepository>,
        assets: Arc<dyn AssetRepository>,
        outbox: Arc<dyn EventOutbox>,
        ids: Arc<dyn IdGenerator>,
        clock: Arc<dyn Clock>,
        policies: PolicySet,
        identity: Arc<dyn IdentityResolver>,
        readiness: Readiness,
        telemetry: Option<Arc<dyn flexcore_ports::telemetry_read::TelemetryReader>>,
        current_state_freshness_millis: i64,
        schedule_records: Arc<dyn ScheduleRecordStore>,
        schedule_revisions: Arc<dyn ScheduleRevisionChainStore>,
        envelope_records: Arc<dyn EnvelopeRecordStore>,
    ) -> Self {
        Self::new(
            Backend::Test {
                tenants,
                portfolios,
                sites,
                assets,
                outbox,
                ids,
                clock,
                schedule_records,
                schedule_revisions,
                envelope_records,
            },
            policies,
            identity,
            readiness,
            telemetry,
            current_state_freshness_millis,
        )
    }

    /// Runs `op` inside one unit of work.
    ///
    /// - **Postgres**: `FlexCoreStore::begin` is SYNCHRONOUS and blocks on
    ///   the adapter's private runtime, so it (and `op`, which calls the
    ///   blocking port methods) runs inside `tokio::task::spawn_blocking`.
    ///   The optional `ctx` is passed to `begin` so the transaction-scoped
    ///   RLS tenant context (`flexcore.tenant_id`) is set before the first
    ///   statement (P1.12 §5.1). On success the unit of work is committed
    ///   via `Arc::try_unwrap` — if any port object outlived the operation
    ///   the unit of work is NOT committed and the request fails closed
    ///   (infrastructure error; the drop-rollback then releases the
    ///   transaction). On any `Err`, the unfinalized unit of work is
    ///   dropped, which spawns the native rollback (P1.10 adapter rule 4).
    /// - **Test**: `op` runs directly against the shared fakes; no
    ///   transaction exists, so there is nothing to commit or roll back.
    pub async fn run_in_uow<T, F>(&self, ctx: Option<TenantContext>, op: F) -> Result<T, ApiError>
    where
        T: Send + 'static,
        F: FnOnce(&UnitOfWork) -> Result<T, ApiError> + Send + 'static,
    {
        match &self.backend {
            Backend::Postgres { store, ids, clock } => {
                let store = store.clone();
                let ids = ids.clone();
                let clock = clock.clone();
                tokio::task::spawn_blocking(move || {
                    let uow = store.begin(ctx.as_ref()).map_err(ApiError::from)?;
                    let shared: Arc<flexcore_postgres_adapter::PgUnitOfWork> = Arc::new(uow);
                    // All five trait-object Arcs share ONE allocation (the
                    // one `PgUnitOfWork`), hence ONE native transaction.
                    let bundle = UnitOfWork {
                        tenants: shared.clone(),
                        portfolios: shared.clone(),
                        sites: shared.clone(),
                        assets: shared.clone(),
                        outbox: shared.clone(),
                        ids,
                        clock,
                        // P3.28: the Phase 3 record stores coerce from the
                        // SAME PgUnitOfWork allocation — their inserts and
                        // the paired outbox rows are one native
                        // transaction (§15.5).
                        schedule_records: shared.clone(),
                        schedule_revisions: shared.clone(),
                        envelope_records: shared.clone(),
                    };
                    let outcome = op(&bundle);
                    // Drop the port objects BEFORE finalizing: the commit
                    // requires the unit-of-work allocation to be uniquely
                    // owned again.
                    drop(bundle);
                    match outcome {
                        Ok(value) => {
                            match Arc::try_unwrap(shared) {
                                Ok(uow) => {
                                    uow.commit().map_err(ApiError::from)?;
                                    Ok(value)
                                }
                                // A port object escaped into `value` (an
                                // API-layer contract violation): fail closed
                                // rather than commit a half-held transaction.
                                Err(_still_shared) => Err(ApiError::infrastructure()),
                            }
                        }
                        // `shared` is dropped here; the adapter's Drop impl
                        // spawns the native rollback (P1.10 rule 4).
                        Err(err) => Err(err),
                    }
                })
                .await
                .map_err(|_| ApiError::infrastructure())?
            }
            Backend::Test {
                tenants,
                portfolios,
                sites,
                assets,
                outbox,
                ids,
                clock,
                schedule_records,
                schedule_revisions,
                envelope_records,
            } => {
                let bundle = UnitOfWork {
                    tenants: tenants.clone(),
                    portfolios: portfolios.clone(),
                    sites: sites.clone(),
                    assets: assets.clone(),
                    outbox: outbox.clone(),
                    ids: ids.clone(),
                    clock: clock.clone(),
                    schedule_records: schedule_records.clone(),
                    schedule_revisions: schedule_revisions.clone(),
                    envelope_records: envelope_records.clone(),
                };
                op(&bundle)
            }
        }
    }
}
