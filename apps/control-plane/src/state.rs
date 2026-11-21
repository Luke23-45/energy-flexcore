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
    /// part of the [`UnitOfWork`] bundle: the Postgres implementation
    /// (`PgTelemetryReader`) is pool-held on the store, not bound to the
    /// request's unit of work — `PgUnitOfWork` is `!Sync`, so no reader
    /// holding one can live in `AppState` as a long-lived `Arc`. Each
    /// trait call therefore opens its OWN tenant-scoped unit of work via
    /// `FlexCoreStore::begin` (RLS `flexcore.tenant_id` set
    /// transaction-locally before the first statement) and commits it
    /// after the read; the request's unit of work (e.g. the
    /// asset-existence check) and the reader's are separate transactions.
    /// A backend failure fails closed (infrastructure error); the Test
    /// backend wires the in-crate fake.
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
    // P11.11 (Phase 11 wave 1, Agent B) — the bounded in-process
    // stored-response replay cache for the Idempotency-Key contract on
    // command-submission POSTs. Per-instance by recorded decision; the
    // Redis-backed shared cache is the recorded multi-instance path,
    // not built here. See `idempotency.rs`.
    pub idempotency: Arc<crate::idempotency::IdempotencyReplayCache>,
    // P11.14 (Phase 11 wave 1, Agent B) — the per-tenant token-bucket and
    // in-flight-cap state for the rate-limit middleware. In-process by
    // recorded decision; budgets multiply per instance until the
    // recorded Redis-backed path lands. See `rate_limit.rs`.
    pub rate_limit: Arc<crate::rate_limit::RateLimitState>,
    // P13.04 (Phase 13 wave 1, Agent A; ADDITIVE, fail-closed) — the
    // server-side step-up challenge service (single-use, time-bounded,
    // purpose-bound challenges; the P13-E-3 DEFAULT is IdP-delegated MFA
    // behind the injected verifier). `None` = not composed → every
    // step-up request fails closed (a recorded denial, never a pass —
    // the same discipline as the `edge_enrollment` seam above). The
    // composition opts in via [`AppState::with_step_up`]; NO composition
    // exists in this repo (no IdP is deployed; P13-E-3 stays OPEN).
    // Decision record: phase-13 `records/04_p13_04_step_up_mfa.md`.
    pub step_up: Option<Arc<crate::stepup::StepUpService>>,
    // FRV1-R8-005 (final review; ADDITIVE) — the P13.09 webhook delivery
    // bookkeeping store (migration 0020), ported over the Postgres
    // adapter's store (`webhook_delivery_pg.rs` implements the port).
    // `None` = not composed. Being composed does NOT mean delivery runs:
    // no enqueue route, no hosting scan loop, and no secret-resolver
    // composition exist in 0.x — the transport seam below is the
    // fail-closed delivery boundary, and the hosting loop + resolver are
    // the recorded follow-ups (P13.09 decision record §5). This field
    // exists so the store is composed and reachable at the recorded seam,
    // not invented later.
    pub webhook_delivery: Option<Arc<dyn crate::webhook_service::WebhookDeliveryStore>>,
    // FIM-F08.03 (Wave 6.5 composition, ADDITIVE) — the OUTBOUND HTTP
    // TRANSPORT seam for webhook delivery, made an explicit state field so
    // the composition can swap it configuration-conditionally. The DEFAULT
    // is exactly the fail-closed posture FRV1-R8-005 recorded:
    // `UnwiredOutboundHttpPost` (refuses every transmission with the named
    // `E_TRANSPORT_UNWIRED` error) — absent webhook-HTTP configuration this
    // field is constructed byte-identically to the pre-FIM implied constant.
    // When the operator sets `FLEXCORE_WEBHOOK_HTTP_ENABLED=true`, the
    // composition root swaps in the LIVE `ReqwestOutboundHttpPost`
    // (`webhook_http.rs`; WRITTEN-UNRUN, live receiver interop is a HUMAN
    // LAB GATE). Swapping the transport does NOT enable delivery: the
    // hosting loop and secret resolver are still the recorded follow-ups,
    // and the dispatcher remains a per-invocation composition
    // (`WebhookDispatcher::new` over these seams), never a long-lived
    // service.
    pub webhook_transport: Arc<dyn crate::webhook_service::OutboundHttpPost>,
    // FIM-F07.02 (Wave 6.5 composition, ADDITIVE, fail-closed) — the SCIM
    // 2.0 provisioning service for the ONE configured connection
    // (`scim.rs`: the service is tenant-pinned by construction). `None` =
    // SCIM not composed → the surface contributes NO routes at all (the
    // router is never mounted — not a 404 fallback). The service's store is
    // the composition's choice; this repo composes the in-memory
    // `InMemoryScimStore` (single-process, bounded by nothing — the
    // Postgres-backed store is the recorded `UnwiredScimStoreBackend`
    // deployment seam, scim_store.rs honesty header).
    pub scim: Option<Arc<crate::scim::ScimService>>,
    // FIM-F07.02 (Wave 6.5 composition, ADDITIVE) — the SHA-256 digest of
    // the SCIM connection's bearer token (the module carries NO
    // authorization; the composed deployment MUST mount it behind
    // authentication — scim.rs honesty header). ONLY the digest is held:
    // the token itself is never stored in state, never logged. `None` (or
    // `scim` set without a digest) = the SCIM router is NOT mounted — the
    // composition refuses to serve an unauthenticated provisioning surface
    // (fail-closed on miscomposition, `routes.rs::fim_router`).
    pub scim_bearer_digest: Option<[u8; 32]>,
    // FIM-F08.01 (Wave 6.5 composition, ADDITIVE, fail-closed) — the
    // machine-identity service (P13.07: client-credentials token
    // issuance/validation/revocation). `None` = not composed → no machine
    // token is ever issued or validated by this process. FIM WAVE 8B: the
    // route surface now EXISTS (`machine_identity_routes.rs`, mounted by
    // `routes.rs::fim_router` ONLY when this slot is composed — which
    // happens only behind the `FLEXCORE_MACHINE_IDENTITY_ENABLED` gate);
    // absent composition the slot stays `None` and the paths do not exist.
    pub machine_identity: Option<Arc<crate::machine_identity::MachineIdentityService>>,
    // FIM-F08.02 (Wave 6.5 composition, ADDITIVE, fail-closed) — the
    // per-tenant API-key service (P13.08: issuance/rotation/authentication/
    // revocation). `None` = not composed → no key is ever issued. FIM WAVE
    // 8B: the route surface now EXISTS (`api_key_routes.rs`, mounted by
    // `routes.rs::fim_router` ONLY when this slot is composed — which
    // happens only behind the `FLEXCORE_API_KEYS_ENABLED` gate); absent
    // composition the slot stays `None` and the paths do not exist.
    pub api_keys: Option<Arc<crate::api_keys::ApiKeyService>>,
    // FIM-F12.05 (Wave 6.5 composition, ADDITIVE, fail-closed) — the OTA
    // cohort-rollout coordinator (`ota_rollout.rs`), behind its mutex (the
    // `OtaReportReceiver` port takes `&mut self` — a state machine over
    // recorded events). `None` = no rollout exists (the shipped default).
    // The receiver port is a TYPED port, not HTTP: no route serves device
    // reports in this binary — that transport is the named remaining
    // wiring step (the coordinator is composed here so the audit chain and
    // cohort state live at the composition root, not invented later).
    pub ota_rollout: Option<Arc<std::sync::Mutex<crate::ota_rollout::OtaRolloutCoordinator>>>,
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
            // P11.11/P11.14 (Agent B): one replay cache and one rate-limit
            // state per process, created here so every variant of the
            // backend (Postgres and Test) carries them.
            idempotency: Arc::new(crate::idempotency::IdempotencyReplayCache::default()),
            rate_limit: Arc::new(crate::rate_limit::RateLimitState::default()),
            // P13.04 (Agent A): no step-up service is composed by default
            // (no IdP exists — fail closed; see the field's doc comment).
            step_up: None,
            // FRV1-R8-005: no webhook-delivery store by default; the
            // Postgres composition root opts in via
            // [`AppState::with_webhook_delivery`].
            webhook_delivery: None,
            // FIM Wave 6.5: the transport seam defaults to the EXACT
            // fail-closed unwired posture (see the field's doc comment);
            // the live reqwest transport composes only behind the
            // webhook-HTTP configuration.
            webhook_transport: Arc::new(crate::webhook_service::UnwiredOutboundHttpPost),
            // FIM-F07.02: SCIM not composed by default (no connection
            // exists; the router is never mounted — see the field docs).
            scim: None,
            scim_bearer_digest: None,
            // FIM-F08.01/FIM-F08.02: no machine-identity or API-key
            // service composed by default (no client, no key — see the
            // field docs).
            machine_identity: None,
            api_keys: None,
            // FIM-F12.05: no OTA rollout exists by default (see the field
            // docs).
            ota_rollout: None,
        }
    }

    /// FRV1-R8-005 composition: wires the webhook-delivery bookkeeping
    /// store (the Postgres adapter's store behind the port impl in
    /// `webhook_delivery_pg.rs`). Composing the store does NOT enable
    /// delivery — see the field's doc comment for the recorded 0.x
    /// posture (no route, no loop, unwired transport).
    pub fn with_webhook_delivery(
        mut self,
        store: Arc<dyn crate::webhook_service::WebhookDeliveryStore>,
    ) -> Self {
        self.webhook_delivery = Some(store);
        self
    }

    /// FIM-F08.03 (Wave 6.5) composition: swaps the webhook transport
    /// seam. The DEFAULT (never composed) stays
    /// [`crate::webhook_service::UnwiredOutboundHttpPost`]; the
    /// configuration-gated composition root swaps in the LIVE
    /// [`crate::webhook_http::ReqwestOutboundHttpPost`] only when the
    /// webhook-HTTP configuration is present. Swapping the transport does
    /// NOT enable delivery (no enqueue route, no hosting loop — see the
    /// field's doc comment).
    pub fn with_webhook_transport(
        mut self,
        transport: Arc<dyn crate::webhook_service::OutboundHttpPost>,
    ) -> Self {
        self.webhook_transport = transport;
        self
    }

    /// FIM-F07.02 (Wave 6.5) composition: wires the SCIM service AND the
    /// digest of its bearer token (the guard material — the token itself
    /// is never held). The pair is atomic BY CONSTRUCTION: there is no way
    /// to compose the service without the guard digest, and
    /// `routes.rs::fim_router` additionally refuses to mount when either
    /// half is missing (fail-closed on miscomposition).
    pub fn with_scim(
        mut self,
        service: Arc<crate::scim::ScimService>,
        bearer_digest: [u8; 32],
    ) -> Self {
        self.scim = Some(service);
        self.scim_bearer_digest = Some(bearer_digest);
        self
    }

    /// FIM-F08.01 (Wave 6.5) composition: wires the machine-identity
    /// service (no route serves it yet — see the field's doc comment).
    pub fn with_machine_identity(
        mut self,
        service: Arc<crate::machine_identity::MachineIdentityService>,
    ) -> Self {
        self.machine_identity = Some(service);
        self
    }

    /// FIM-F08.02 (Wave 6.5) composition: wires the API-key service (no
    /// route serves it yet — see the field's doc comment).
    pub fn with_api_keys(mut self, service: Arc<crate::api_keys::ApiKeyService>) -> Self {
        self.api_keys = Some(service);
        self
    }

    /// FIM-F12.05 (Wave 6.5) composition: wires the OTA rollout
    /// coordinator (the receiver port is typed, not HTTP — see the
    /// field's doc comment).
    pub fn with_ota_rollout(
        mut self,
        coordinator: Arc<std::sync::Mutex<crate::ota_rollout::OtaRolloutCoordinator>>,
    ) -> Self {
        self.ota_rollout = Some(coordinator);
        self
    }

    /// P13.04 composition: wires the server-side step-up service (the
    /// deployment's challenge store, verifier, id generator, and clock —
    /// the P13-E-3 DEFAULT is IdP-delegated MFA behind the verifier; the
    /// shipped default when NOT composed is a fail-closed denial on every
    /// step-up request, never a pass). See `stepup.rs` and the phase-13
    /// record `records/04_p13_04_step_up_mfa.md`.
    pub fn with_step_up(mut self, service: Arc<crate::stepup::StepUpService>) -> Self {
        self.step_up = Some(service);
        self
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
