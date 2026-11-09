// P1.07 — In-memory test fakes: the contract-test implementations that
//         prove the application depends only on the port traits (P1.07
//         step 5; P1.08 acceptance: application tests run against fakes,
//         not against real infrastructure).
//
// **TEST DOUBLES — NEVER PRODUCTION.** This module is compiled
// unconditionally and is public so flexcore-application tests can compose
// the same fakes; it must never be wired into a deployment. Nothing here
// persists, locks correctly across processes, or scales — that is not its
// job. Its job is to make the port contracts *observable*:
//
// - [`FakeTenantRepository`] / [`FakePortfolioRepository`] /
//   [`FakeSiteRepository`] / [`FakeAssetRepository`] — in-memory repositories
//   enforcing the tenant-scoping rules (P0.16 rules 1, 5, 7): writes that
//   would cross the caller's tenant scope are refused
//   ([`RepositoryError::TenantMismatch`]); cross-tenant reads read as
//   "not found" — existence is never leaked (the same behaviour the P1.13
//   adapter must implement).
// - [`FakeEventOutbox`] — an in-memory outbox honouring the
//   idempotency-key uniqueness rule and the delivery-state transitions.
// - [`FakeWriteJournal`] — the shared, ordered journal every fake appends
//   to. This is the P1.08 ordering evidence: after a use case runs, the
//   journal shows the state-change op and the outbox-append op adjacent,
//   both before the single `Commit` marker the (simulated) composition
//   root records — i.e. state change and outbox entry registered in one
//   unit of work. The fakes deliberately do NOT emulate rollback: the
//   all-or-nothing obligation belongs to the real adapter and its
//   atomicity test (P1.13).
// - [`FixedClock`] — a deterministic, advanceable clock.
// - [`CountingIdGenerator`] — a deterministic, strictly increasing
//   generator that counts every generation.
// - [`FakeTelemetryReader`] — an in-memory `TelemetryReader` (P2.12 read
//   surface: query, current state, quarantine) for the API tests; the
//   Postgres implementation of the port is the recorded P2.06-adjacent gap.
// - [`FakeTelemetryIngestStore`] / [`FakeIngestionInbox`] /
//   [`FakeTelemetrySource`] — in-memory doubles of the P2.09/P2.10 ingest
//   surface (duplicate classification, quarantine evidence, inbox
//   check/record split, and a decode-controllable provider) so the P2.09
//   dedup module and the P2.10 ingestion pipeline tests run without any
//   infrastructure.
//
// Recorded limitation (crate README): the tenant-scoped fakes enforce
// scoping against the domain `TenantContext`, and no code outside
// flexcore-domain can construct one today (P1.06 claims boundary — see
// `tenant_context_source`). Their scoping behaviour is therefore exercised
// by the application tests only once the domain exposes the claims
// constructor; the expected test bodies are documented in the
// flexcore-application README. The [`FakeTenantRepository`] (no context by
// recorded decision) and the outbox/clock/ID fakes are fully exercised
// below.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use flexcore_contracts::ids::FlexUuid;
use flexcore_contracts::time_model::{EventTime, IngestTime};
use flexcore_domain::entities::{Asset, Portfolio, Site, Tenant};
use flexcore_domain::ids::{AssetId, PortfolioId, SiteId, TenantId};
use flexcore_domain::tenant_context::TenantContext;

use crate::clock::Clock;
use crate::event_outbox::{EventOutbox, EventOutboxError, OutboxWrite, PendingOutboxEntry};
use crate::id_generator::IdGenerator;
use crate::repository::{
    AssetRepository, PortfolioRepository, RepositoryError, SiteRepository, TenantRepository,
};
use crate::state_change::{StateChangeOperation, StateChangeReceipt, entities};
use crate::telemetry_read::{
    QuarantinedTelemetry, TelemetryCursor, TelemetryPage, TelemetryReadError, TelemetryReadQuery,
    TelemetryReader, sort_key,
};
use crate::telemetry_ingest::{
    IngestDeliveryOutcome, IngestInboxError, IngestionInbox, TelemetryIngestError,
    TelemetryIngestItem, TelemetryIngestOutcome, TelemetryIngestStore, TelemetryLatestRecord,
    TelemetryQuarantineWrite,
};
use crate::telemetry_source::{
    NormalizedTelemetry, ProviderDeclaration, TelemetryQuery, TelemetrySource,
    TelemetrySourceError, TelemetrySourceHealth, TelemetrySubscription, TelemetryQuantity,
};

// --- The shared write journal -----------------------------------------------

/// One ordered operation recorded in the [`FakeWriteJournal`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FakeJournalOp {
    /// A repository write was recorded (entity, operation, tenant scope).
    StateChange {
        /// The entity written (a [`crate::state_change::entities`] name).
        entity: String,
        /// The operation applied.
        operation: StateChangeOperation,
        /// The tenant scope (`None` only for the tenant registry root).
        tenant_scope: Option<TenantId>,
    },
    /// An outbox append was recorded (key + the state change it is paired
    /// with, so the ordering test can assert the pairing).
    OutboxAppend {
        /// The idempotency key of the appended record.
        idempotency_key: String,
        /// The entity the paired state change named.
        entity: String,
        /// The tenant scope of the paired state change.
        tenant_scope: Option<TenantId>,
    },
    /// The (simulated) composition root committed the unit of work.
    Commit,
}

/// The shared, ordered journal the fakes append to — the P1.08 ordering
/// evidence for the §6.3 step-1 rule (state change + outbox record in one
/// unit of work). Cheap to clone (shared `Arc`).
#[derive(Clone, Default)]
pub struct FakeWriteJournal {
    ops: Arc<Mutex<Vec<FakeJournalOp>>>,
}

impl FakeWriteJournal {
    /// Creates an empty journal.
    pub fn new() -> Self {
        Self::default()
    }

    /// A snapshot of the recorded operations, in order.
    pub fn ops(&self) -> Vec<FakeJournalOp> {
        self.ops
            .lock()
            .expect("poisoned mutex in test fake")
            .clone()
    }

    /// Records the `Commit` marker — called by whatever simulates the
    /// composition root's unit-of-work boundary (in tests: the test itself,
    /// right after the use case returns).
    pub fn record_commit(&self) {
        self.push(FakeJournalOp::Commit);
    }

    fn push(&self, op: FakeJournalOp) {
        self.ops
            .lock()
            .expect("poisoned mutex in test fake")
            .push(op);
    }
}

// --- Tenant repository fake --------------------------------------------------

/// In-memory [`TenantRepository`]: keyed by tenant identifier, with the
/// identity-provider-reference uniqueness rule enforced (the recorded
/// duplicate-tenant anchor). No `TenantContext` — by the recorded root-
/// aggregate exception, not by omission.
#[derive(Default)]
pub struct FakeTenantRepository {
    rows: Mutex<HashMap<TenantId, Tenant>>,
    journal: FakeWriteJournal,
}

impl FakeTenantRepository {
    /// Creates the fake; writes are journalled into `journal`.
    pub fn new(journal: FakeWriteJournal) -> Self {
        Self {
            rows: Mutex::new(HashMap::new()),
            journal,
        }
    }

    /// How many tenants are stored.
    pub fn len(&self) -> usize {
        self.rows.lock().expect("poisoned mutex in test fake").len()
    }

    /// Whether the fake stores no tenants.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl TenantRepository for FakeTenantRepository {
    fn insert(&self, tenant: &Tenant) -> Result<StateChangeReceipt, RepositoryError> {
        let mut rows = self.rows.lock().expect("poisoned mutex in test fake");
        if rows.contains_key(&tenant.tenant_id()) {
            return Err(RepositoryError::Conflict {
                entity: entities::TENANTS,
                reason: "tenant identifier already registered".to_string(),
            });
        }
        if rows
            .values()
            .any(|t| t.identity_provider_reference() == tenant.identity_provider_reference())
        {
            return Err(RepositoryError::Conflict {
                entity: entities::TENANTS,
                reason: "identity provider reference already registered".to_string(),
            });
        }
        rows.insert(tenant.tenant_id(), tenant.clone());
        self.journal.push(FakeJournalOp::StateChange {
            entity: entities::TENANTS.to_string(),
            operation: StateChangeOperation::Insert,
            tenant_scope: None,
        });
        Ok(StateChangeReceipt::new(
            entities::TENANTS.to_string(),
            StateChangeOperation::Insert,
            None,
        ))
    }

    fn update(&self, tenant: &Tenant) -> Result<StateChangeReceipt, RepositoryError> {
        let mut rows = self.rows.lock().expect("poisoned mutex in test fake");
        if !rows.contains_key(&tenant.tenant_id()) {
            return Err(RepositoryError::NotFound {
                entity: entities::TENANTS,
            });
        }
        rows.insert(tenant.tenant_id(), tenant.clone());
        self.journal.push(FakeJournalOp::StateChange {
            entity: entities::TENANTS.to_string(),
            operation: StateChangeOperation::Update,
            tenant_scope: None,
        });
        Ok(StateChangeReceipt::new(
            entities::TENANTS.to_string(),
            StateChangeOperation::Update,
            None,
        ))
    }

    fn find_by_id(&self, tenant_id: TenantId) -> Result<Option<Tenant>, RepositoryError> {
        let rows = self.rows.lock().expect("poisoned mutex in test fake");
        Ok(rows.get(&tenant_id).cloned())
    }

    fn find_by_identity_provider_reference(
        &self,
        identity_provider_reference: &str,
    ) -> Result<Option<Tenant>, RepositoryError> {
        let rows = self.rows.lock().expect("poisoned mutex in test fake");
        Ok(rows
            .values()
            .find(|t| t.identity_provider_reference() == identity_provider_reference)
            .cloned())
    }

    fn list_all(&self) -> Result<Vec<Tenant>, RepositoryError> {
        let rows = self.rows.lock().expect("poisoned mutex in test fake");
        let mut all: Vec<Tenant> = rows.values().cloned().collect();
        all.sort_by_key(|t| t.tenant_id());
        Ok(all)
    }

    fn delete(&self, tenant_id: TenantId) -> Result<bool, RepositoryError> {
        let mut rows = self.rows.lock().expect("poisoned mutex in test fake");
        let deleted = rows.remove(&tenant_id).is_some();
        if deleted {
            self.journal.push(FakeJournalOp::StateChange {
                entity: entities::TENANTS.to_string(),
                operation: StateChangeOperation::Delete,
                tenant_scope: None,
            });
        }
        Ok(deleted)
    }
}

// --- Tenant-scoped repository fakes ------------------------------------------
//
// Shared shape: every write first checks the entity's tenant against the
// context (`TenantMismatch` on a foreign entity — the caller already holds
// it, so there is no existence to hide), then the identifier/uniqueness
// rules; every read is scoped to the context's tenant, and a foreign row
// reads as absent (`None` / `false`) — no existence leak. All four fakes
// implement the same rules; the struct bodies differ only in the row type.

macro_rules! tenant_scoped_fake {
    (
        $(#[$meta:meta])*
        $fake:ident, $row:ty, $row_id:ident
    ) => {
        $(#[$meta])*
        #[derive(Default)]
        pub struct $fake {
            rows: Mutex<HashMap<$row_id, $row>>,
            journal: FakeWriteJournal,
        }

        impl $fake {
            /// Creates the fake; writes are journalled into `journal`.
            pub fn new(journal: FakeWriteJournal) -> Self {
                Self {
                    rows: Mutex::new(HashMap::new()),
                    journal,
                }
            }

            /// How many rows are stored (any tenant — inspection helper
            /// for tests, not a port method).
            pub fn len(&self) -> usize {
                self.rows.lock().expect("poisoned mutex in test fake").len()
            }

            /// Whether the fake stores no rows.
            pub fn is_empty(&self) -> bool {
                self.len() == 0
            }
        }
    };
}

tenant_scoped_fake! {
    /// In-memory portfolio repository: tenant-scoped per P0.16 rules 1
    /// and 5; the identifier is the uniqueness key and the tenant-scoped
    /// portfolio name is the recorded natural-key rule. Implements
    /// [`PortfolioRepository`].
    FakePortfolioRepository, Portfolio, PortfolioId
}

tenant_scoped_fake! {
    /// In-memory site repository: tenant-scoped per P0.16 rules 1, 5,
    /// and 7 (edge identities stay tenant-scoped). Implements
    /// [`SiteRepository`].
    FakeSiteRepository, Site, SiteId
}

tenant_scoped_fake! {
    /// In-memory asset repository: tenant-scoped per P0.16 rules 1
    /// and 5; the identifier is the uniqueness key and the asset name
    /// within its site is the recorded natural-key rule. Implements
    /// [`AssetRepository`].
    FakeAssetRepository, Asset, AssetId
}

impl PortfolioRepository for FakePortfolioRepository {
    fn insert(
        &self,
        ctx: &TenantContext,
        portfolio: &Portfolio,
    ) -> Result<StateChangeReceipt, RepositoryError> {
        let mut rows = self.rows.lock().expect("poisoned mutex in test fake");
        if portfolio.tenant_id() != ctx.tenant_id() {
            return Err(RepositoryError::TenantMismatch {
                entity: entities::PORTFOLIOS,
            });
        }
        if rows.contains_key(&portfolio.portfolio_id()) {
            return Err(RepositoryError::Conflict {
                entity: entities::PORTFOLIOS,
                reason: "portfolio identifier already registered".to_string(),
            });
        }
        if rows
            .values()
            .any(|p| p.name() == portfolio.name())
        {
            return Err(RepositoryError::Conflict {
                entity: entities::PORTFOLIOS,
                reason: "portfolio name already registered for this tenant".to_string(),
            });
        }
        rows.insert(portfolio.portfolio_id(), portfolio.clone());
        self.journal.push(FakeJournalOp::StateChange {
            entity: entities::PORTFOLIOS.to_string(),
            operation: StateChangeOperation::Insert,
            tenant_scope: Some(ctx.tenant_id()),
        });
        Ok(StateChangeReceipt::new(
            entities::PORTFOLIOS.to_string(),
            StateChangeOperation::Insert,
            Some(ctx.tenant_id()),
        ))
    }

    fn update(
        &self,
        ctx: &TenantContext,
        portfolio: &Portfolio,
    ) -> Result<StateChangeReceipt, RepositoryError> {
        let mut rows = self.rows.lock().expect("poisoned mutex in test fake");
        if portfolio.tenant_id() != ctx.tenant_id() {
            return Err(RepositoryError::TenantMismatch {
                entity: entities::PORTFOLIOS,
            });
        }
        if !rows.contains_key(&portfolio.portfolio_id()) {
            return Err(RepositoryError::NotFound {
                entity: entities::PORTFOLIOS,
            });
        }
        rows.insert(portfolio.portfolio_id(), portfolio.clone());
        self.journal.push(FakeJournalOp::StateChange {
            entity: entities::PORTFOLIOS.to_string(),
            operation: StateChangeOperation::Update,
            tenant_scope: Some(ctx.tenant_id()),
        });
        Ok(StateChangeReceipt::new(
            entities::PORTFOLIOS.to_string(),
            StateChangeOperation::Update,
            Some(ctx.tenant_id()),
        ))
    }

    fn find_by_id(
        &self,
        ctx: &TenantContext,
        portfolio_id: PortfolioId,
    ) -> Result<Option<Portfolio>, RepositoryError> {
        let rows = self.rows.lock().expect("poisoned mutex in test fake");
        Ok(rows
            .get(&portfolio_id)
            .filter(|p| p.tenant_id() == ctx.tenant_id())
            .cloned())
    }

    fn list_for_tenant(&self, ctx: &TenantContext) -> Result<Vec<Portfolio>, RepositoryError> {
        let rows = self.rows.lock().expect("poisoned mutex in test fake");
        let mut owned: Vec<Portfolio> = rows
            .values()
            .filter(|p| p.tenant_id() == ctx.tenant_id())
            .cloned()
            .collect();
        owned.sort_by_key(|p| p.portfolio_id());
        Ok(owned)
    }

    fn delete(
        &self,
        ctx: &TenantContext,
        portfolio_id: PortfolioId,
    ) -> Result<bool, RepositoryError> {
        let mut rows = self.rows.lock().expect("poisoned mutex in test fake");
        let exists_for_tenant = rows
            .get(&portfolio_id)
            .is_some_and(|p| p.tenant_id() == ctx.tenant_id());
        let deleted = exists_for_tenant && rows.remove(&portfolio_id).is_some();
        if deleted {
            self.journal.push(FakeJournalOp::StateChange {
                entity: entities::PORTFOLIOS.to_string(),
                operation: StateChangeOperation::Delete,
                tenant_scope: Some(ctx.tenant_id()),
            });
        }
        Ok(deleted)
    }
}

impl SiteRepository for FakeSiteRepository {
    fn insert(
        &self,
        ctx: &TenantContext,
        site: &Site,
    ) -> Result<StateChangeReceipt, RepositoryError> {
        let mut rows = self.rows.lock().expect("poisoned mutex in test fake");
        if site.tenant_id() != ctx.tenant_id() {
            return Err(RepositoryError::TenantMismatch {
                entity: entities::SITES,
            });
        }
        if rows.contains_key(&site.site_id()) {
            return Err(RepositoryError::Conflict {
                entity: entities::SITES,
                reason: "site identifier already registered".to_string(),
            });
        }
        if rows.values().any(|s| s.edge_identity() == site.edge_identity()) {
            return Err(RepositoryError::Conflict {
                entity: entities::SITES,
                reason: "site edge identity already registered for this tenant".to_string(),
            });
        }
        rows.insert(site.site_id(), site.clone());
        self.journal.push(FakeJournalOp::StateChange {
            entity: entities::SITES.to_string(),
            operation: StateChangeOperation::Insert,
            tenant_scope: Some(ctx.tenant_id()),
        });
        Ok(StateChangeReceipt::new(
            entities::SITES.to_string(),
            StateChangeOperation::Insert,
            Some(ctx.tenant_id()),
        ))
    }

    fn update(
        &self,
        ctx: &TenantContext,
        site: &Site,
    ) -> Result<StateChangeReceipt, RepositoryError> {
        let mut rows = self.rows.lock().expect("poisoned mutex in test fake");
        if site.tenant_id() != ctx.tenant_id() {
            return Err(RepositoryError::TenantMismatch {
                entity: entities::SITES,
            });
        }
        if !rows.contains_key(&site.site_id()) {
            return Err(RepositoryError::NotFound {
                entity: entities::SITES,
            });
        }
        rows.insert(site.site_id(), site.clone());
        self.journal.push(FakeJournalOp::StateChange {
            entity: entities::SITES.to_string(),
            operation: StateChangeOperation::Update,
            tenant_scope: Some(ctx.tenant_id()),
        });
        Ok(StateChangeReceipt::new(
            entities::SITES.to_string(),
            StateChangeOperation::Update,
            Some(ctx.tenant_id()),
        ))
    }

    fn find_by_id(
        &self,
        ctx: &TenantContext,
        site_id: SiteId,
    ) -> Result<Option<Site>, RepositoryError> {
        let rows = self.rows.lock().expect("poisoned mutex in test fake");
        Ok(rows
            .get(&site_id)
            .filter(|s| s.tenant_id() == ctx.tenant_id())
            .cloned())
    }

    fn list_for_tenant(&self, ctx: &TenantContext) -> Result<Vec<Site>, RepositoryError> {
        let rows = self.rows.lock().expect("poisoned mutex in test fake");
        let mut owned: Vec<Site> = rows
            .values()
            .filter(|s| s.tenant_id() == ctx.tenant_id())
            .cloned()
            .collect();
        owned.sort_by_key(|s| s.site_id());
        Ok(owned)
    }

    fn delete(&self, ctx: &TenantContext, site_id: SiteId) -> Result<bool, RepositoryError> {
        let mut rows = self.rows.lock().expect("poisoned mutex in test fake");
        let exists_for_tenant = rows
            .get(&site_id)
            .is_some_and(|s| s.tenant_id() == ctx.tenant_id());
        let deleted = exists_for_tenant && rows.remove(&site_id).is_some();
        if deleted {
            self.journal.push(FakeJournalOp::StateChange {
                entity: entities::SITES.to_string(),
                operation: StateChangeOperation::Delete,
                tenant_scope: Some(ctx.tenant_id()),
            });
        }
        Ok(deleted)
    }
}

impl AssetRepository for FakeAssetRepository {
    fn insert(
        &self,
        ctx: &TenantContext,
        asset: &Asset,
    ) -> Result<StateChangeReceipt, RepositoryError> {
        let mut rows = self.rows.lock().expect("poisoned mutex in test fake");
        if asset.tenant_id() != ctx.tenant_id() {
            return Err(RepositoryError::TenantMismatch {
                entity: entities::ASSETS,
            });
        }
        if rows.contains_key(&asset.asset_id()) {
            return Err(RepositoryError::Conflict {
                entity: entities::ASSETS,
                reason: "asset identifier already registered".to_string(),
            });
        }
        if rows.values().any(|a| a.site_id() == asset.site_id() && a.name() == asset.name()) {
            return Err(RepositoryError::Conflict {
                entity: entities::ASSETS,
                reason: "asset name already registered for this site".to_string(),
            });
        }
        rows.insert(asset.asset_id(), asset.clone());
        self.journal.push(FakeJournalOp::StateChange {
            entity: entities::ASSETS.to_string(),
            operation: StateChangeOperation::Insert,
            tenant_scope: Some(ctx.tenant_id()),
        });
        Ok(StateChangeReceipt::new(
            entities::ASSETS.to_string(),
            StateChangeOperation::Insert,
            Some(ctx.tenant_id()),
        ))
    }

    fn update(
        &self,
        ctx: &TenantContext,
        asset: &Asset,
    ) -> Result<StateChangeReceipt, RepositoryError> {
        let mut rows = self.rows.lock().expect("poisoned mutex in test fake");
        if asset.tenant_id() != ctx.tenant_id() {
            return Err(RepositoryError::TenantMismatch {
                entity: entities::ASSETS,
            });
        }
        if !rows.contains_key(&asset.asset_id()) {
            return Err(RepositoryError::NotFound {
                entity: entities::ASSETS,
            });
        }
        rows.insert(asset.asset_id(), asset.clone());
        self.journal.push(FakeJournalOp::StateChange {
            entity: entities::ASSETS.to_string(),
            operation: StateChangeOperation::Update,
            tenant_scope: Some(ctx.tenant_id()),
        });
        Ok(StateChangeReceipt::new(
            entities::ASSETS.to_string(),
            StateChangeOperation::Update,
            Some(ctx.tenant_id()),
        ))
    }

    fn find_by_id(
        &self,
        ctx: &TenantContext,
        asset_id: AssetId,
    ) -> Result<Option<Asset>, RepositoryError> {
        let rows = self.rows.lock().expect("poisoned mutex in test fake");
        Ok(rows
            .get(&asset_id)
            .filter(|a| a.tenant_id() == ctx.tenant_id())
            .cloned())
    }

    fn list_for_tenant(&self, ctx: &TenantContext) -> Result<Vec<Asset>, RepositoryError> {
        let rows = self.rows.lock().expect("poisoned mutex in test fake");
        let mut owned: Vec<Asset> = rows
            .values()
            .filter(|a| a.tenant_id() == ctx.tenant_id())
            .cloned()
            .collect();
        owned.sort_by_key(|a| a.asset_id());
        Ok(owned)
    }

    fn delete(&self, ctx: &TenantContext, asset_id: AssetId) -> Result<bool, RepositoryError> {
        let mut rows = self.rows.lock().expect("poisoned mutex in test fake");
        let exists_for_tenant = rows
            .get(&asset_id)
            .is_some_and(|a| a.tenant_id() == ctx.tenant_id());
        let deleted = exists_for_tenant && rows.remove(&asset_id).is_some();
        if deleted {
            self.journal.push(FakeJournalOp::StateChange {
                entity: entities::ASSETS.to_string(),
                operation: StateChangeOperation::Delete,
                tenant_scope: Some(ctx.tenant_id()),
            });
        }
        Ok(deleted)
    }
}

// --- Event outbox fake -------------------------------------------------------

#[derive(Debug, Clone)]
struct FakeOutboxRow {
    write: OutboxWrite,
    attempt_count: u32,
    delivered: bool,
    failed: bool,
}

/// In-memory [`EventOutbox`]: honours idempotency-key uniqueness, keeps the
/// delivery state per entry, and journals every append as an
/// [`FakeJournalOp::OutboxAppend`] carrying the paired state change — the
/// other half of the P1.08 ordering evidence.
#[derive(Default)]
pub struct FakeEventOutbox {
    rows: Mutex<Vec<FakeOutboxRow>>,
    journal: FakeWriteJournal,
}

impl FakeEventOutbox {
    /// Creates the fake; appends are journalled into `journal`.
    pub fn new(journal: FakeWriteJournal) -> Self {
        Self {
            rows: Mutex::new(Vec::new()),
            journal,
        }
    }

    /// How many entries are stored (any delivery state).
    pub fn len(&self) -> usize {
        self.rows.lock().expect("poisoned mutex in test fake").len()
    }

    /// Whether the fake stores no entries.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// A snapshot of every appended write, in insertion order (test
    /// inspection helper).
    pub fn writes(&self) -> Vec<OutboxWrite> {
        self.rows
            .lock()
            .expect("poisoned mutex in test fake")
            .iter()
            .map(|r| r.write.clone())
            .collect()
    }
}

impl EventOutbox for FakeEventOutbox {
    fn append(&self, write: OutboxWrite) -> Result<(), EventOutboxError> {
        let mut rows = self.rows.lock().expect("poisoned mutex in test fake");
        if rows
            .iter()
            .any(|r| r.write.idempotency_key() == write.idempotency_key())
        {
            return Err(EventOutboxError::DuplicateKey {
                idempotency_key: write.idempotency_key().to_string(),
            });
        }
        self.journal.push(FakeJournalOp::OutboxAppend {
            idempotency_key: write.idempotency_key().to_string(),
            entity: write.state_change().entity().to_string(),
            tenant_scope: write.state_change().tenant_scope(),
        });
        rows.push(FakeOutboxRow {
            write,
            attempt_count: 0,
            delivered: false,
            failed: false,
        });
        Ok(())
    }

    fn pending(&self, limit: u32) -> Result<Vec<PendingOutboxEntry>, EventOutboxError> {
        let rows = self.rows.lock().expect("poisoned mutex in test fake");
        Ok(rows
            .iter()
            .filter(|r| !r.delivered && !r.failed)
            .take(limit as usize)
            .map(|r| {
                PendingOutboxEntry::new(
                    r.write.idempotency_key().to_string(),
                    r.write.destination_subject().to_string(),
                    r.write.payload().to_string(),
                    r.attempt_count,
                    r.write.created_at(),
                )
            })
            .collect())
    }

    fn record_attempt(&self, idempotency_key: &str) -> Result<(), EventOutboxError> {
        let mut rows = self.rows.lock().expect("poisoned mutex in test fake");
        let row = rows
            .iter_mut()
            .find(|r| r.write.idempotency_key() == idempotency_key)
            .ok_or_else(|| EventOutboxError::UnknownKey {
                idempotency_key: idempotency_key.to_string(),
            })?;
        row.attempt_count += 1;
        Ok(())
    }

    fn mark_delivered(&self, idempotency_key: &str) -> Result<(), EventOutboxError> {
        let mut rows = self.rows.lock().expect("poisoned mutex in test fake");
        let row = rows
            .iter_mut()
            .find(|r| r.write.idempotency_key() == idempotency_key)
            .ok_or_else(|| EventOutboxError::UnknownKey {
                idempotency_key: idempotency_key.to_string(),
            })?;
        row.delivered = true;
        Ok(())
    }

    fn mark_failed(&self, idempotency_key: &str) -> Result<(), EventOutboxError> {
        let mut rows = self.rows.lock().expect("poisoned mutex in test fake");
        let row = rows
            .iter_mut()
            .find(|r| r.write.idempotency_key() == idempotency_key)
            .ok_or_else(|| EventOutboxError::UnknownKey {
                idempotency_key: idempotency_key.to_string(),
            })?;
        row.failed = true;
        Ok(())
    }
}

// --- Clock fake --------------------------------------------------------------

/// A deterministic [`Clock`]: returns a fixed instant (advanceable), so
/// application tests are fully reproducible. Panics at construction on an
/// out-of-range millisecond value — a test-only convenience; the trait's
/// return types cannot fail.
pub struct FixedClock {
    millis: Mutex<i64>,
}

impl FixedClock {
    /// Pins the clock at `millis` since the Unix epoch (UTC).
    ///
    /// Panics if the value is not a valid UTC timestamp — a test-fake
    /// convenience, documented here rather than hidden.
    pub fn at_unix_millis(millis: i64) -> Self {
        EventTime::from_unix_millis(millis)
            .unwrap_or_else(|e| panic!("FixedClock needs a valid UTC timestamp: {e}"));
        Self {
            millis: Mutex::new(millis),
        }
    }

    /// Advances the fixed instant by `delta` milliseconds (may be negative).
    pub fn advance_millis(&self, delta: i64) {
        *self.millis.lock().expect("poisoned mutex in test fake") += delta;
    }

    /// The current fixed instant in milliseconds since the Unix epoch.
    pub fn unix_millis(&self) -> i64 {
        *self.millis.lock().expect("poisoned mutex in test fake")
    }
}

impl Clock for FixedClock {
    fn now_event_time(&self) -> EventTime {
        EventTime::from_unix_millis(self.unix_millis())
            .expect("the fixed instant was validated at construction")
    }

    fn now_ingest_time(&self) -> IngestTime {
        IngestTime::from_unix_millis(self.unix_millis())
            .expect("the fixed instant was validated at construction")
    }
}

// --- ID-generator fake -------------------------------------------------------

/// A deterministic [`IdGenerator`]: yields a strictly increasing sequence
/// of non-nil identifiers and counts every generation (hence the name).
pub struct CountingIdGenerator {
    generated: Mutex<u64>,
}

impl CountingIdGenerator {
    /// Creates the generator; the first identifier is deterministic
    /// (`...0001` in big-endian byte form) and each call advances it.
    pub fn new() -> Self {
        Self {
            generated: Mutex::new(0),
        }
    }

    /// How many identifiers this generator has produced.
    pub fn generated_count(&self) -> u64 {
        *self.generated.lock().expect("poisoned mutex in test fake")
    }
}

impl Default for CountingIdGenerator {
    fn default() -> Self {
        Self::new()
    }
}

impl IdGenerator for CountingIdGenerator {
    fn new_uuid(&self) -> FlexUuid {
        let mut generated = self.generated.lock().expect("poisoned mutex in test fake");
        *generated += 1;
        FlexUuid::from_bytes((*generated as u128).to_be_bytes())
    }
}

// --- Telemetry-reader fake (P2.12/P2.17/P2.18) --------------------------------

/// In-memory [`TelemetryReader`]: stores canonical records and quarantine
/// entries so the API-layer tests can exercise the telemetry read surface
/// without the (parallel-tracked) storage adapter. Enforces the port's
/// tenant-scoping rule: records whose tenant differs from the query context
/// never exist at this boundary (no existence leak, P0.16 rule 5).
///
/// The current-state rule is implemented exactly as recorded on the port:
/// latest `event_time` per quantity (never latest arrival), ties broken by
/// the higher `source_sequence`.
#[derive(Default)]
pub struct FakeTelemetryReader {
    records: Mutex<Vec<NormalizedTelemetry>>,
    quarantine: Mutex<Vec<QuarantinedTelemetry>>,
}

impl FakeTelemetryReader {
    /// Creates an empty reader.
    pub fn new() -> Self {
        Self::default()
    }

    /// Seeds one canonical record (test input; the ingestion pipeline owns
    /// the write side in production).
    pub fn insert(&self, record: NormalizedTelemetry) {
        self.records
            .lock()
            .expect("poisoned mutex in test fake")
            .push(record);
    }

    /// Seeds many records (convenience).
    pub fn seed(&self, records: Vec<NormalizedTelemetry>) {
        for r in records {
            self.insert(r);
        }
    }

    /// Seeds one quarantine entry (test input).
    pub fn quarantine(&self, entry: QuarantinedTelemetry) {
        self.quarantine
            .lock()
            .expect("poisoned mutex in test fake")
            .push(entry);
    }

    /// A snapshot of the stored records (test inspection helper).
    pub fn stored(&self) -> Vec<NormalizedTelemetry> {
        self.records
            .lock()
            .expect("poisoned mutex in test fake")
            .clone()
    }

    /// A snapshot of the stored quarantine entries (test inspection helper).
    pub fn quarantined(&self) -> Vec<QuarantinedTelemetry> {
        self.quarantine
            .lock()
            .expect("poisoned mutex in test fake")
            .clone()
    }
}

impl TelemetryReader for FakeTelemetryReader {
    fn query(
        &self,
        ctx: &TenantContext,
        query: TelemetryReadQuery,
    ) -> Result<TelemetryPage, TelemetryReadError> {
        let records = self.records.lock().expect("poisoned mutex in test fake");
        let mut matches: Vec<&NormalizedTelemetry> = records
            .iter()
            .filter(|r| r.tenant_id() == ctx.tenant_id())
            .filter(|r| r.asset_id() == query.asset())
            .filter(|r| r.event_time() >= query.from() && r.event_time() <= query.until())
            .filter(|r| {
                query
                    .quantities()
                    .map(|qs| qs.contains(&r.quantity()))
                    .unwrap_or(true)
            })
            .filter(|r| query.quality().map(|q| r.quality() == q).unwrap_or(true))
            .filter(|r| {
                query
                    .cursor()
                    .map(|c| sort_key(r).as_str() > c.key())
                    .unwrap_or(true)
            })
            .collect();
        matches.sort_by_key(|r| sort_key(r));
        let limit = query.limit() as usize;
        let page: Vec<NormalizedTelemetry> =
            matches.into_iter().take(limit).cloned().collect();
        let next_cursor = if page.len() == limit {
            page.last().map(TelemetryCursor::for_record)
        } else {
            None
        };
        Ok(TelemetryPage::new(page, next_cursor))
    }

    fn latest_state(
        &self,
        ctx: &TenantContext,
        asset_id: AssetId,
    ) -> Result<Vec<NormalizedTelemetry>, TelemetryReadError> {
        let records = self.records.lock().expect("poisoned mutex in test fake");
        // Latest by event_time (NOT arrival), ties by higher source_sequence
        // (the port contract; P2.18).
        let mut best: Vec<(TelemetryQuantity, &NormalizedTelemetry)> = Vec::new();
        for r in records
            .iter()
            .filter(|r| r.tenant_id() == ctx.tenant_id() && r.asset_id() == asset_id)
        {
            match best.iter_mut().find(|(q, _)| *q == r.quantity()) {
                Some((_, current)) => {
                    if (r.event_time().unix_millis(), r.source_sequence())
                        > (current.event_time().unix_millis(), current.source_sequence())
                    {
                        *current = r;
                    }
                }
                None => best.push((r.quantity(), r)),
            }
        }
        Ok(best
            .into_iter()
            .map(|(_, r)| r.clone())
            .collect())
    }

    fn list_quarantine(
        &self,
        ctx: &TenantContext,
    ) -> Result<Vec<QuarantinedTelemetry>, TelemetryReadError> {
        Ok(self
            .quarantine
            .lock()
            .expect("poisoned mutex in test fake")
            .iter()
            .filter(|e| e.tenant_id() == ctx.tenant_id())
            .cloned()
            .collect())
    }
}

// --- Ingest fakes (P2.09/P2.10) -----------------------------------------------
//
// NOTE (recorded gap, same pattern as the tenant-scoped fakes above): these
// fakes enforce the tenant-scoping rules against the domain `TenantContext`
// and construct canonical records through the ports' own Result
// constructors, so they are fully exercisable by the P2.09 dedup tests in
// flexcore-application (the claims constructor exists in the domain now).

/// In-memory [`TelemetryIngestStore`]: classifies exact vs conflicting
/// duplicates exactly per the port rule (same identity + byte-identical
/// content → acknowledged; same identity + any difference → conflicting),
/// records quarantine writes, and serves the latest-by-`event_time`
/// snapshot (ties: higher `source_sequence` — the P2.09 read rule).
#[derive(Default)]
pub struct FakeTelemetryIngestStore {
    records: Mutex<Vec<NormalizedTelemetry>>,
    quarantine: Mutex<Vec<TelemetryQuarantineWrite>>,
}

impl FakeTelemetryIngestStore {
    /// Creates an empty store.
    pub fn new() -> Self {
        Self::default()
    }

    /// A snapshot of the stored records (test inspection helper).
    pub fn stored(&self) -> Vec<NormalizedTelemetry> {
        self.records.lock().expect("poisoned mutex in test fake").clone()
    }

    /// A snapshot of the quarantine entries (test inspection helper).
    pub fn quarantined(&self) -> Vec<TelemetryQuarantineWrite> {
        self.quarantine.lock().expect("poisoned mutex in test fake").clone()
    }
}

impl TelemetryIngestStore for FakeTelemetryIngestStore {
    fn insert_batch(
        &self,
        ctx: &TenantContext,
        records: &[NormalizedTelemetry],
    ) -> Result<Vec<TelemetryIngestItem>, TelemetryIngestError> {
        let mut items = Vec::with_capacity(records.len());
        let mut store = self.records.lock().expect("poisoned mutex in test fake");
        for (index, incoming) in records.iter().enumerate() {
            if incoming.tenant_id() != ctx.tenant_id() {
                return Err(TelemetryIngestError::TenantMismatch);
            }
            let outcome = match store
                .iter()
                .find(|stored| stored.asset_id() == incoming.asset_id()
                    && stored.event_time() == incoming.event_time()
                    && stored.source_sequence() == incoming.source_sequence()
                    && stored.quantity() == incoming.quantity())
            {
                Some(stored) if duplicate_content_equal(stored, incoming) => {
                    TelemetryIngestOutcome::ExactDuplicateAcknowledged
                }
                Some(_) => TelemetryIngestOutcome::ConflictingDuplicate,
                None => {
                    store.push(incoming.clone());
                    TelemetryIngestOutcome::Stored
                }
            };
            items.push(TelemetryIngestItem { index, outcome });
        }
        Ok(items)
    }

    fn quarantine(
        &self,
        ctx: &TenantContext,
        entry: &TelemetryQuarantineWrite,
    ) -> Result<(), TelemetryIngestError> {
        if entry.tenant_id != ctx.tenant_id() {
            return Err(TelemetryIngestError::TenantMismatch);
        }
        self.quarantine
            .lock()
            .expect("poisoned mutex in test fake")
            .push(entry.clone());
        Ok(())
    }

    fn latest_record(
        &self,
        ctx: &TenantContext,
        asset_id: AssetId,
        quantity: TelemetryQuantity,
    ) -> Result<Option<TelemetryLatestRecord>, TelemetryIngestError> {
        let store = self.records.lock().expect("poisoned mutex in test fake");
        Ok(store
            .iter()
            .filter(|r| {
                r.tenant_id() == ctx.tenant_id()
                    && r.asset_id() == asset_id
                    && r.quantity() == quantity
            })
            .max_by(|a, b| {
                (a.event_time(), a.source_sequence()).cmp(&(b.event_time(), b.source_sequence()))
            })
            .map(|r| TelemetryLatestRecord {
                event_time: r.event_time(),
                quality: r.quality(),
            }))
    }
}

/// Whether two same-identity records carry identical content (the exact-
/// duplicate rule; every value-bearing and identity field must match —
/// only the surrogate row handle may differ, and the port type has none).
fn duplicate_content_equal(stored: &NormalizedTelemetry, incoming: &NormalizedTelemetry) -> bool {
    stored == incoming
}

/// In-memory [`IngestionInbox`]: the check/record split over a key set.
/// Forced redelivery in tests = call the pipeline twice against the same
/// fake (the first call's recorded deliveries are seen by the second).
#[derive(Default)]
pub struct FakeIngestionInbox {
    recorded: Mutex<std::collections::HashSet<(String, String)>>,
}

impl FakeIngestionInbox {
    /// Creates an empty inbox.
    pub fn new() -> Self {
        Self::default()
    }

    /// How many distinct deliveries are recorded (test inspection helper).
    pub fn len(&self) -> usize {
        self.recorded.lock().expect("poisoned mutex in test fake").len()
    }

    /// Whether the inbox is empty (test inspection helper).
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl IngestionInbox for FakeIngestionInbox {
    fn is_recorded(&self, consumer: &str, idempotency_key: &str) -> Result<bool, IngestInboxError> {
        Ok(self
            .recorded
            .lock()
            .expect("poisoned mutex in test fake")
            .contains(&(consumer.to_string(), idempotency_key.to_string())))
    }

    fn record(
        &self,
        consumer: &str,
        idempotency_key: &str,
    ) -> Result<IngestDeliveryOutcome, IngestInboxError> {
        let inserted = self
            .recorded
            .lock()
            .expect("poisoned mutex in test fake")
            .insert((consumer.to_string(), idempotency_key.to_string()));
        if inserted {
            Ok(IngestDeliveryOutcome::FirstDelivery)
        } else {
            Ok(IngestDeliveryOutcome::AlreadyRecorded)
        }
    }
}

/// In-memory [`TelemetrySource`] fake: emits a preset record set for any
/// non-empty frame, fails decoding (with a recorded reason) when armed, so
/// the decode-failure and source-auth paths of the ingestion pipeline are
/// testable without a provider implementation (P2.14 wires the simulator).
pub struct FakeTelemetrySource {
    declaration: ProviderDeclaration,
    records: Mutex<Vec<NormalizedTelemetry>>,
    fail_decode: std::sync::atomic::AtomicBool,
}

impl FakeTelemetrySource {
    /// Builds the fake with the given provider declaration and the preset
    /// record set every non-empty frame decodes to.
    pub fn new(declaration: ProviderDeclaration, records: Vec<NormalizedTelemetry>) -> Self {
        Self {
            declaration,
            records: Mutex::new(records),
            fail_decode: std::sync::atomic::AtomicBool::new(false),
        }
    }

    /// Arms/disarms the decode-failure path (test control).
    pub fn set_fail_decode(&self, fail: bool) {
        self.fail_decode
            .store(fail, std::sync::atomic::Ordering::SeqCst);
    }

    /// Replaces the preset record set (test control).
    pub fn set_records(&self, records: Vec<NormalizedTelemetry>) {
        *self.records.lock().expect("poisoned mutex in test fake") = records;
    }
}

impl TelemetrySource for FakeTelemetrySource {
    fn provider_declaration(&self) -> &ProviderDeclaration {
        &self.declaration
    }

    fn subscribe(
        &self,
        _query: &TelemetryQuery,
    ) -> Result<TelemetrySubscription, TelemetrySourceError> {
        Ok(TelemetrySubscription::from_handle(1))
    }

    fn decode(
        &self,
        _subscription: &TelemetrySubscription,
        payload: &[u8],
    ) -> Result<Vec<NormalizedTelemetry>, TelemetrySourceError> {
        if self.fail_decode.load(std::sync::atomic::Ordering::SeqCst) {
            return Err(TelemetrySourceError::DecodeFailed {
                reason: "the fake source was armed to fail decoding".to_string(),
            });
        }
        if payload.is_empty() {
            return Err(TelemetrySourceError::DecodeFailed {
                reason: "empty frame".to_string(),
            });
        }
        let records = self.records.lock().expect("poisoned mutex in test fake");
        if records.is_empty() {
            return Err(TelemetrySourceError::DecodeFailed {
                reason: "the fake source holds no records".to_string(),
            });
        }
        Ok(records.clone())
    }

    fn health(&self) -> TelemetrySourceHealth {
        TelemetrySourceHealth::Healthy
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state_change::entities;

    const UUID_TENANT_A: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f7a01";
    const UUID_TENANT_B: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f7a02";

    fn tenant(name: &str, idp: &str, uuid_text: &str) -> Tenant {
        Tenant::try_new(
            TenantId::new(FlexUuid::parse(uuid_text).expect("valid test uuid")),
            name.to_string(),
            None,
            idp.to_string(),
            "profile:default".to_string(),
            EventTime::from_unix_millis(1_000).expect("valid timestamp"),
        )
        .expect("valid tenant")
    }

    // --- FakeTenantRepository ------------------------------------------------

    #[test]
    fn tenant_fake_stores_and_finds_by_id_and_idp_reference() {
        let journal = FakeWriteJournal::new();
        let repo = FakeTenantRepository::new(journal.clone());
        let t = tenant("Acme", "idp:acme", UUID_TENANT_A);

        let receipt = repo.insert(&t).expect("insert succeeds");
        assert_eq!(receipt.entity(), entities::TENANTS);
        assert_eq!(receipt.tenant_scope(), None, "the tenant root is tenant-less");

        assert_eq!(
            repo.find_by_id(t.tenant_id()).expect("find succeeds"),
            Some(t.clone())
        );
        assert_eq!(
            repo.find_by_identity_provider_reference("idp:acme")
                .expect("find succeeds"),
            Some(t)
        );
        assert_eq!(
            repo.find_by_identity_provider_reference("idp:other")
                .expect("find succeeds"),
            None
        );
        assert_eq!(repo.len(), 1);
    }

    #[test]
    fn tenant_fake_rejects_duplicate_identifier_and_duplicate_idp_reference() {
        let journal = FakeWriteJournal::new();
        let repo = FakeTenantRepository::new(journal.clone());
        let t = tenant("Acme", "idp:acme", UUID_TENANT_A);
        let _ = repo.insert(&t).expect("first insert succeeds");

        // Same identifier, different everything else.
        let same_id = tenant("Other", "idp:other", UUID_TENANT_A);
        let err = repo.insert(&same_id).unwrap_err();
        assert!(
            matches!(err, RepositoryError::Conflict { .. }),
            "duplicate identifier must conflict: {err}"
        );
        // Different identifier, same identity-provider reference.
        let same_idp = tenant("Acme Two", "idp:acme", UUID_TENANT_B);
        let err = repo.insert(&same_idp).unwrap_err();
        assert!(
            matches!(err, RepositoryError::Conflict { .. }),
            "duplicate idp reference must conflict: {err}"
        );
        assert_eq!(repo.len(), 1, "neither conflicting insert stored a row");
    }

    #[test]
    fn tenant_fake_update_delete_and_listing_behave() {
        let journal = FakeWriteJournal::new();
        let repo = FakeTenantRepository::new(journal.clone());
        let t = tenant("Acme", "idp:acme", UUID_TENANT_A);
        repo.insert(&t).expect("insert succeeds");

        let updated = tenant("Acme Renamed", "idp:acme", UUID_TENANT_A);
        let _ = repo.update(&updated).expect("update succeeds");
        assert_eq!(
            repo.find_by_id(updated.tenant_id()).expect("find succeeds").expect("row exists").name(),
            "Acme Renamed"
        );

        let missing = tenant("Ghost", "idp:ghost", UUID_TENANT_B);
        assert!(
            matches!(
                repo.update(&missing),
                Err(RepositoryError::NotFound { .. })
            ),
            "updating an unknown identifier is NotFound"
        );

        assert_eq!(repo.list_all().expect("list succeeds").len(), 1);
        assert!(repo.delete(updated.tenant_id()).expect("delete succeeds"));
        assert!(!repo.delete(updated.tenant_id()).expect("delete succeeds"));
        assert!(repo.is_empty());
    }

    #[test]
    fn tenant_fake_journals_every_write() {
        let journal = FakeWriteJournal::new();
        let repo = FakeTenantRepository::new(journal.clone());
        let t = tenant("Acme", "idp:acme", UUID_TENANT_A);
        let _ = repo.insert(&t).expect("insert succeeds");
        repo.delete(t.tenant_id()).expect("delete succeeds");
        journal.record_commit();

        assert_eq!(
            journal.ops(),
            vec![
                FakeJournalOp::StateChange {
                    entity: "tenants".to_string(),
                    operation: StateChangeOperation::Insert,
                    tenant_scope: None,
                },
                FakeJournalOp::StateChange {
                    entity: "tenants".to_string(),
                    operation: StateChangeOperation::Delete,
                    tenant_scope: None,
                },
                FakeJournalOp::Commit,
            ]
        );
    }

    // --- FakeEventOutbox -------------------------------------------------------

    fn sample_write(key: &str) -> OutboxWrite {
        let receipt = StateChangeReceipt::new(
            entities::TENANTS.to_string(),
            StateChangeOperation::Insert,
            None,
        );
        OutboxWrite::new(
            receipt,
            key.to_string(),
            "flexcore.v1.audit.operator_action_recorded".to_string(),
            "{\"serialized\": true}".to_string(),
            EventTime::from_unix_millis(1_000).expect("valid timestamp"),
        )
        .expect("valid write")
    }

    #[test]
    fn outbox_fake_appends_reads_pending_and_enforces_key_uniqueness() {
        let journal = FakeWriteJournal::new();
        let outbox = FakeEventOutbox::new(journal.clone());

        outbox.append(sample_write("key-1")).expect("append succeeds");
        outbox.append(sample_write("key-2")).expect("append succeeds");
        let err = outbox.append(sample_write("key-1")).unwrap_err();
        assert_eq!(
            err,
            EventOutboxError::DuplicateKey {
                idempotency_key: "key-1".to_string()
            }
        );
        assert_eq!(outbox.len(), 2, "the duplicate append stored nothing");

        let pending = outbox.pending(10).expect("pending succeeds");
        assert_eq!(pending.len(), 2);
        assert_eq!(pending[0].idempotency_key(), "key-1", "insertion order");
        assert_eq!(pending[0].attempt_count(), 0);

        // limit truncates, order preserved.
        assert_eq!(outbox.pending(1).expect("pending succeeds").len(), 1);
    }

    #[test]
    fn outbox_fake_lifecycle_attempt_then_delivered_removes_from_pending() {
        let journal = FakeWriteJournal::new();
        let outbox = FakeEventOutbox::new(journal.clone());
        outbox.append(sample_write("key-1")).expect("append succeeds");
        outbox.append(sample_write("key-2")).expect("append succeeds");

        outbox.record_attempt("key-1").expect("attempt recorded");
        assert_eq!(
            outbox.pending(10).expect("pending succeeds")[0].attempt_count(),
            1,
            "the attempt count is visible to the publisher"
        );

        // Delivered only after broker acknowledgement — and only then out of
        // the pending set.
        outbox.mark_delivered("key-1").expect("mark succeeds");
        let pending = outbox.pending(10).expect("pending succeeds");
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].idempotency_key(), "key-2");

        // Failed entries also leave the pending set but stay in the store
        // for investigation (never silently dropped).
        outbox.record_attempt("key-2").expect("attempt recorded");
        outbox.mark_failed("key-2").expect("mark succeeds");
        assert!(outbox.pending(10).expect("pending succeeds").is_empty());
        assert_eq!(outbox.len(), 2, "failed entries are retained");
    }

    #[test]
    fn outbox_fake_unknown_keys_are_reported() {
        let outbox = FakeEventOutbox::new(FakeWriteJournal::new());
        for op in ["record_attempt", "mark_delivered", "mark_failed"] {
            let err = match op {
                "record_attempt" => outbox.record_attempt("ghost").unwrap_err(),
                "mark_delivered" => outbox.mark_delivered("ghost").unwrap_err(),
                _ => outbox.mark_failed("ghost").unwrap_err(),
            };
            assert_eq!(
                err,
                EventOutboxError::UnknownKey {
                    idempotency_key: "ghost".to_string()
                },
                "{op} on an unknown key must be reported"
            );
        }
    }

    #[test]
    fn outbox_fake_journals_the_paired_state_change_with_every_append() {
        let journal = FakeWriteJournal::new();
        let outbox = FakeEventOutbox::new(journal.clone());
        let write = sample_write("key-1");
        outbox.append(write.clone()).expect("append succeeds");

        assert_eq!(
            journal.ops(),
            vec![FakeJournalOp::OutboxAppend {
                idempotency_key: "key-1".to_string(),
                entity: write.state_change().entity().to_string(),
                tenant_scope: write.state_change().tenant_scope(),
            }]
        );
    }

    // --- FixedClock ------------------------------------------------------------

    #[test]
    fn fixed_clock_is_deterministic_and_advanceable() {
        let clock = FixedClock::at_unix_millis(1_000);
        assert_eq!(clock.now_event_time().unix_millis(), 1_000);
        assert_eq!(clock.now_ingest_time().unix_millis(), 1_000);
        clock.advance_millis(500);
        assert_eq!(clock.now_event_time().unix_millis(), 1_500);
        assert_eq!(clock.unix_millis(), 1_500);
        clock.advance_millis(-200);
        assert_eq!(clock.now_ingest_time().unix_millis(), 1_300);
    }

    // --- CountingIdGenerator -----------------------------------------------------

    #[test]
    fn counting_generator_yields_distinct_non_nil_ids_and_counts() {
        let gen = CountingIdGenerator::new();
        assert_eq!(gen.generated_count(), 0);
        let a = gen.new_uuid();
        let b = gen.new_tenant_id();
        let c = gen.new_asset_id();
        assert_ne!(a, FlexUuid::NIL);
        assert_ne!(a, b.uuid());
        assert_ne!(b.uuid(), c.uuid());
        assert_eq!(gen.generated_count(), 3);
    }

    #[test]
    fn counting_generator_sequence_is_deterministic() {
        let gen = CountingIdGenerator::new();
        let first = gen.new_uuid();
        let second = gen.new_uuid();
        let expected_first = FlexUuid::from_bytes(1u128.to_be_bytes());
        let expected_second = FlexUuid::from_bytes(2u128.to_be_bytes());
        assert_eq!(first, expected_first);
        assert_eq!(second, expected_second);
    }

    // --- Tenant-scoped fakes -----------------------------------------------------
    //
    // NOTE (recorded gap, see `tenant_context_source` and the crate
    // README): the portfolio/site/asset fakes take the domain
    // `TenantContext` on every method, and no code outside flexcore-domain
    // can construct one today (P1.06 claims boundary). Their scoping
    // behaviour is therefore exercised by the application tests once the
    // domain exposes the claims constructor; the expected test bodies are
    // documented in the flexcore-application README. What is provable here
    // without a context is that the fakes exist, construct, and start
    // empty.

    #[test]
    fn tenant_scoped_fakes_construct_empty() {
        let journal = FakeWriteJournal::new();
        let portfolios = FakePortfolioRepository::new(journal.clone());
        let sites = FakeSiteRepository::new(journal.clone());
        let assets = FakeAssetRepository::new(journal.clone());
        assert!(portfolios.is_empty());
        assert!(sites.is_empty());
        assert!(assets.is_empty());
        // The journal is shared, not per-fake state.
        assert!(journal.ops().is_empty());
    }
}
