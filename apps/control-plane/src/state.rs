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
use flexcore_storage::audit_read::AuditEventReader;
use flexcore_ports::clock::Clock;
use flexcore_ports::event_outbox::EventOutbox;
use flexcore_ports::id_generator::IdGenerator;
use flexcore_ports::repository::{
    AssetRepository, PortfolioRepository, SiteRepository, TenantRepository,
};
use flexcore_storage::command_store::CommandRecordStore;
use flexcore_storage::envelope_store::EnvelopeRecordStore;
use flexcore_storage::feasibility_check::FeasibilityCheckStore;
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
    /// ([`crate::phase3_fakes`]). P4.30: the Phase 4 record stores (the
    /// command records and the feasibility-check evidence) join the bundle
    /// as the in-crate fakes ([`crate::phase4_fakes`]). P4.36: the
    /// audit-events reader joins the bundle as the in-crate fake as well.
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
        command_records: Arc<dyn CommandRecordStore>,
        feasibility_checks: Arc<dyn FeasibilityCheckStore>,
        audit_events: Arc<dyn AuditEventReader>,
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
    /// The dispatch-command record store (P4.10, migration 0016; the
    /// §15.2 dispatch-commands read resource). In the Postgres backend it
    /// coerces from the SAME `PgUnitOfWork` allocation (one transaction).
    pub command_records: Arc<dyn CommandRecordStore>,
    /// The feasibility-check evidence record store (P4.30, migration
    /// 0017; the §15.2 feasibility-checks read resource). Same
    /// one-transaction discipline.
    pub feasibility_checks: Arc<dyn FeasibilityCheckStore>,
    /// The audit-events reader (P4.36, migration 0008 read half; the
    /// §15.2 audit-events read resource). SELECT-only; same
    /// one-transaction discipline.
    pub audit_events: Arc<dyn AuditEventReader>,
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
    /// The in-process live-update bus (P5.25, `routes_live.rs`): the
    /// composition root's NATS forwarder publishes `flexcore.v1.>` messages
    /// into it; the `GET /api/v1/live` SSE handlers subscribe and filter
    /// per caller tenant. Deliberately NOT transaction-bound — it carries
    /// forwarded broker messages, not unit-of-work state.
    pub live: Arc<crate::routes_live::LiveEventBus>,
    // P6.06 — the enrollment surface (ADDITIVE; both fail-closed by
    // default). Deliberately NOT in the [`UnitOfWork`] bundle: the
    // enrollment route is a provisioning surface whose store opens its own
    // tenant-scoped unit of work per call (the Postgres implementation on
    // `FlexCoreStore`), not a per-request registry read inside the
    // command/telemetry transaction.
    /// The edge device-identity registry + enrollment journal store
    /// (migration 0018). `None` = not composed → the enrollment route
    /// fails closed (infrastructure error, never a fake success).
    pub edge_enrollment: Option<Arc<dyn flexcore_storage::edge_enrollment::EdgeEnrollmentStore>>,
    /// The provisioning-token validator seam (the enrollment credential;
    /// the shipped default denies everything — no token validation service
    /// exists in 0.x; a real validator is composed per deployment).
    pub enrollment_tokens: Arc<dyn crate::routes_edge::ProvisioningTokenValidator>,
    // P6.25 (ADDITIVE) — the edge-status source seam behind
    // `GET /api/v1/edge/status`: the deployment composes its edge-status
    // forwarder here. The shipped default reports NOTHING — every
    // operational field renders as an explicit gap, never a plausible
    // healthy value (the P6.25 risk row).
    pub edge_status: Arc<dyn crate::routes_edge::EdgeStatusSource>,
    // P7.25 (ADDITIVE) — the plugin-inventory source seam behind
    // `GET /api/v1/plugins`: the deployment composes its host/health
    // forwarder here. The shipped default serves the RECORDED inventory of
    // the shipped manifest/profile files with every RUNTIME fact (lifecycle
    // state, health) an explicit gap — never a plausible "ready"/"healthy".
    pub plugins_inventory: Arc<dyn crate::routes_plugins::PluginInventorySource>,
    // P8.27 (ADDITIVE) — the research-status source seam behind
    // `GET /api/v1/research/status`: the deployment composes its
    // evaluation/shadow forwarder here. The shipped default serves the
    // RECORDED Phase 8 research status (module registry, recorded OPEN
    // blockers, mode lock) with every RUN fact (proposals, shadow
    // comparisons, statistics) an explicit gap — never a plausible result.
    pub research_status: Arc<dyn crate::routes_research::ResearchStatusSource>,
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
            // P5.25: one bus per state; the SSE route and the composition
            // root's forwarder share it through this Arc.
            live: Arc::new(crate::routes_live::LiveEventBus::new()),
            // P6.06: both enrollment seams default to FAIL CLOSED (no
            // store → infrastructure error; the deny-all token validator
            // → 401). Composition opts in via the builders below.
            edge_enrollment: None,
            enrollment_tokens: Arc::new(crate::routes_edge::DenyAllEnrollmentTokens),
            // P6.25: the status source defaults to the honest "nothing
            // reported" seam (explicit gaps in the status read).
            edge_status: Arc::new(crate::routes_edge::NoEdgeStatusReported),
            // P7.25: the inventory source defaults to the RECORDED shipped
            // inventory (runtime facts are explicit gaps — no host reporter
            // is composed in 0.x).
            plugins_inventory: Arc::new(crate::routes_plugins::RecordedShippedInventory),
            // P8.27: the research-status source defaults to the RECORDED
            // Phase 8 status (run facts are explicit gaps — no research
            // run exists; proposals are EMPTY, comparisons NO_RUN).
            research_status: Arc::new(crate::routes_research::RecordedResearchStatus),
        }
    }

    /// P8.27 composition: wires the research-status source (the
    /// deployment's evaluation/shadow forwarder).
    pub fn with_research_status(
        mut self,
        source: Arc<dyn crate::routes_research::ResearchStatusSource>,
    ) -> Self {
        self.research_status = source;
        self
    }

    /// P7.25 composition: wires the plugin-inventory source (the
    /// deployment's host/health forwarder).
    pub fn with_plugins_inventory(
        mut self,
        source: Arc<dyn crate::routes_plugins::PluginInventorySource>,
    ) -> Self {
        self.plugins_inventory = source;
        self
    }

    /// P6.25 composition: wires the edge-status source (the deployment's
    /// edge-status forwarder).
    pub fn with_edge_status(
        mut self,
        source: Arc<dyn crate::routes_edge::EdgeStatusSource>,
    ) -> Self {
        self.edge_status = source;
        self
    }

    /// P6.06 composition: wires the edge-enrollment store (the Postgres
    /// `EdgeEnrollmentStore` on `FlexCoreStore`, or the Test fake).
    pub fn with_edge_enrollment(
        mut self,
        store: Arc<dyn flexcore_storage::edge_enrollment::EdgeEnrollmentStore>,
    ) -> Self {
        self.edge_enrollment = Some(store);
        self
    }

    /// P6.06 composition: wires the provisioning-token validator.
    pub fn with_enrollment_tokens(
        mut self,
        validator: Arc<dyn crate::routes_edge::ProvisioningTokenValidator>,
    ) -> Self {
        self.enrollment_tokens = validator;
        self
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
        Self::for_tests_with_phase4(
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
            Arc::new(crate::phase4_fakes::FakeCommandRecordStore::new()),
            Arc::new(crate::phase4_fakes::FakeFeasibilityCheckStore::new()),
        )
    }

    /// Test composition with explicit Phase 4 record stores AND the
    /// audit-events reader (P4.36): the same bundle as
    /// [`Self::for_tests_with_phase4`] with the audit-events fake (or a
    /// failing injection) supplied by the test, so audit-read tests can
    /// seed and inspect it. Used by `tests/api_phase4.rs`.
    #[allow(clippy::too_many_arguments)]
    pub fn for_tests_with_phase4_audit(
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
        command_records: Arc<dyn CommandRecordStore>,
        feasibility_checks: Arc<dyn FeasibilityCheckStore>,
        audit_events: Arc<dyn AuditEventReader>,
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
                command_records,
                feasibility_checks,
                audit_events,
            },
            policies,
            identity,
            readiness,
            telemetry,
            current_state_freshness_millis,
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
        // P4.30: the Phase 4 record stores default to fresh in-crate
        // fakes ([`crate::phase4_fakes`]) — use
        // [`Self::for_tests_with_phase4`] when a test needs to inspect or
        // inject them.
        Self::for_tests_with_phase4(
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
            schedule_records,
            schedule_revisions,
            envelope_records,
            Arc::new(crate::phase4_fakes::FakeCommandRecordStore::new()),
            Arc::new(crate::phase4_fakes::FakeFeasibilityCheckStore::new()),
        )
    }

    /// Test composition with explicit Phase 4 record stores (P4.30): the
    /// same bundle as [`Self::for_tests_with_phase3`] with the
    /// command-record and feasibility-check fakes supplied by the test.
    /// P4.36: the audit-events reader defaults to a fresh in-crate fake
    /// ([`crate::phase4_fakes`]) — use [`Self::for_tests_with_phase4_audit`]
    /// when a test needs to seed or inspect it.
    /// Used by `tests/api_phase4.rs`.
    #[allow(clippy::too_many_arguments)]
    pub fn for_tests_with_phase4(
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
        command_records: Arc<dyn CommandRecordStore>,
        feasibility_checks: Arc<dyn FeasibilityCheckStore>,
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
                command_records,
                feasibility_checks,
                audit_events: Arc::new(crate::phase4_fakes::FakeAuditEventReader::new()),
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
                        // transaction (§15.5). P4.30: the Phase 4 record
                        // stores (command records, feasibility-check
                        // evidence) join the same one-transaction
                        // discipline. P4.36: the audit-events reader
                        // (SELECT-only) joins it as well.
                        schedule_records: shared.clone(),
                        schedule_revisions: shared.clone(),
                        envelope_records: shared.clone(),
                        command_records: shared.clone(),
                        feasibility_checks: shared.clone(),
                        audit_events: shared.clone(),
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
                command_records,
                feasibility_checks,
                audit_events,
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
                    command_records: command_records.clone(),
                    feasibility_checks: feasibility_checks.clone(),
                    audit_events: audit_events.clone(),
                };
                op(&bundle)
            }
        }
    }
}
