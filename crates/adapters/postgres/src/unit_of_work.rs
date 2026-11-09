// P1.13 — PgUnitOfWork: the unit-of-work-scoped port bundle.
//
// One unit of work = one native `sqlx::Transaction` (P1.10 adapter contract
// rule 1). Every port call on this type — repository writes, the outbox
// append, the audit append, and the tenant-scoped reads — executes inside
// that one transaction, all-or-nothing (rule 2). `commit` and `rollback`
// finalize it exactly once and surface the storage crate's
// `FinalizedUnitOfWork` (rule 3); dropping an unfinalized unit of work
// spawns a rollback of the native transaction (rule 4, P1.10 Drop
// obligation — the enforcement is the adapter's atomicity test).
//
// The transactional-outbox rule (report §6.3 step 1) holds by construction
// here: the repository write that produced the `StateChangeReceipt` and the
// `EventOutbox::append` that consumes it run on the SAME transaction, so an
// injected or real failure leaves neither the state change nor the outbox
// row — the mechanical proof is `tests/integration.rs`
// (`outbox_write_is_atomic_with_the_state_change`).
//
// Tenant scoping: `FlexCoreStore::begin(Some(&ctx))` sets the
// transaction-scoped `flexcore.tenant_id` (SET LOCAL semantics) before the
// first statement; the P1.12 §5.1 policies fail closed without it. On top
// of RLS, every tenant-scoped query also filters by the context's tenant
// and every write refuses a foreign-tenant entity with
// `RepositoryError::TenantMismatch` (P1.07 adapter obligations).
//
// Read reconstruction: the ports return full domain entities, and the P1.06
// constructors derive child tenant scoping from the parent entity, so
// decoding a Portfolio/Site/Asset row loads its parent chain from the same
// transaction (tenant rows are cheap, indexed lookups; RLS already scopes
// every fetch to the caller's tenant).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use flexcore_audit::record::{AuditError, AuditRecord};
use flexcore_audit::sink::AuditSink;
use flexcore_contracts::ids::FlexUuid;
use flexcore_contracts::time_model::{EventTime, SiteTimezone};
use flexcore_contracts::units::{ActivePowerMW, EnergyMWh};
use flexcore_domain::entities::{Asset, Portfolio, Site, Tenant};
use flexcore_domain::ids::{AssetId, PortfolioId, SiteId, TenantId};
use flexcore_domain::tenant_context::TenantContext;
use flexcore_ports::backend_error::BackendError;
use flexcore_ports::event_outbox::{EventOutbox, EventOutboxError, OutboxWrite, PendingOutboxEntry};
use flexcore_ports::repository::{
    AssetRepository, PortfolioRepository, RepositoryError, SiteRepository, TenantRepository,
};
use flexcore_ports::state_change::{entities, StateChangeOperation, StateChangeReceipt};
use flexcore_storage::unit_of_work::{FinalizedUnitOfWork, UnitOfWork};
use serde_json::Value;
use sqlx::postgres::{PgRow, Postgres};
use sqlx::Transaction;
use time::OffsetDateTime;
use tokio::runtime::Runtime;
use tokio::sync::Mutex as AsyncMutex;

use crate::enum_map::{asset_type_from_text, asset_type_text};
use crate::error::{
    decode_failure, map_backend, map_repository_error, referential_failure, unit_of_work_finalized,
};
use crate::outbox;
use crate::uuid_map::{uuid_from_text, uuid_text};

/// The native transaction behind one unit of work. Opened from the pool, so
/// it carries the `'static` lifetime. Shared with the telemetry repository
/// (P2.06), which runs inside the same unit-of-work transaction.
pub(crate) type Tx = Transaction<'static, Postgres>;

/// Column projections (uuid columns selected back as `::text` — the
/// recorded FlexUuid mapping).
const TENANT_SELECT: &str = "SELECT tenant_id::text AS tenant_id, name, description, \
     idp_reference, policy_profile, created_at FROM tenants";
const PORTFOLIO_SELECT: &str = "SELECT portfolio_id::text AS portfolio_id, \
     tenant_id::text AS tenant_id, name, description, operator, market_region, \
     aggregation_rules, created_at FROM portfolios";
const SITE_SELECT: &str = "SELECT site_id::text AS site_id, portfolio_id::text AS portfolio_id, \
     tenant_id::text AS tenant_id, name, description, location, timezone, pcc, \
     grid_model_reference, edge_identity, created_at FROM sites";
const ASSET_SELECT: &str = "SELECT asset_id::text AS asset_id, site_id::text AS site_id, \
     tenant_id::text AS tenant_id, name, description, asset_type, vendor_model, \
     rated_power_mw, rated_energy_mwh, phase, bus_reference, capability_profile, \
     created_at FROM assets";

/// One unit of work: the port bundle over one native transaction.
pub struct PgUnitOfWork {
    tx: Arc<AsyncMutex<Option<Tx>>>,
    runtime: Arc<Runtime>,
    unit: UnitOfWork,
    finalized: AtomicBool,
}

impl PgUnitOfWork {
    /// Assembles the unit of work around an open native transaction
    /// (called by [`crate::FlexCoreStore::begin`], which also set the
    /// tenant context when a `TenantContext` was supplied).
    pub(crate) fn assemble(tx: Tx, runtime: Arc<Runtime>) -> Self {
        Self {
            tx: Arc::new(AsyncMutex::new(Some(tx))),
            runtime,
            unit: UnitOfWork::begin_labelled("flexcore-postgres"),
            finalized: AtomicBool::new(false),
        }
    }

    /// Commits the native transaction. Consumes the unit of work: exactly
    /// one finalization is possible (P1.10 rule 3).
    pub fn commit(mut self) -> Result<FinalizedUnitOfWork, BackendError> {
        self.finalized.store(true, Ordering::SeqCst);
        let tx = Arc::clone(&self.tx);
        let runtime = Arc::clone(&self.runtime);
        let unit = self.unit.clone();
        drop(self);
        runtime.block_on(async move {
            let mut guard = tx.lock().await;
            match guard.take() {
                Some(tx) => tx.commit().await.map_err(map_backend),
                None => Err(unit_of_work_finalized()),
            }
        })?;
        Ok(unit.commit())
    }

    /// Rolls the native transaction back. Consumes the unit of work.
    pub fn rollback(mut self) -> Result<FinalizedUnitOfWork, BackendError> {
        self.finalized.store(true, Ordering::SeqCst);
        let tx = Arc::clone(&self.tx);
        let runtime = Arc::clone(&self.runtime);
        let unit = self.unit.clone();
        drop(self);
        runtime.block_on(async move {
            let mut guard = tx.lock().await;
            match guard.take() {
                Some(tx) => tx.rollback().await.map_err(map_backend),
                None => Err(unit_of_work_finalized()),
            }
        })?;
        Ok(unit.rollback())
    }
}

/// P1.10 adapter contract rule 4: an unfinalized unit of work rolls its
/// native transaction back on drop. The rollback runs on the store's
/// runtime (a `Drop` handler cannot await); the transaction was never
/// committed, so its writes can never become visible either way.
impl Drop for PgUnitOfWork {
    fn drop(&mut self) {
        if !self.finalized.load(Ordering::SeqCst) {
            let tx = Arc::clone(&self.tx);
            let runtime = Arc::clone(&self.runtime);
            runtime.spawn(async move {
                if let Some(tx) = tx.lock().await.take() {
                    let _ = tx.rollback().await;
                }
            });
        }
    }
}

/// The transaction reference behind the lock; fails closed once finalized.
pub(crate) async fn tx_ref(guard: &mut Option<Tx>) -> Result<&mut Tx, BackendError> {
    guard.as_mut().ok_or_else(unit_of_work_finalized)
}

// --- Row decoding helpers ----------------------------------------------------

fn col_str(row: &PgRow, column: &'static str) -> Result<String, RepositoryError> {
    row.try_get::<String, _>(column)
        .map_err(|_| RepositoryError::Backend(decode_failure(column)))
}

fn col_opt_str(row: &PgRow, column: &'static str) -> Result<Option<String>, RepositoryError> {
    row.try_get::<Option<String>, _>(column)
        .map_err(|_| RepositoryError::Backend(decode_failure(column)))
}

fn col_f64(row: &PgRow, column: &'static str) -> Result<f64, RepositoryError> {
    row.try_get::<f64, _>(column)
        .map_err(|_| RepositoryError::Backend(decode_failure(column)))
}

fn col_opt_f64(row: &PgRow, column: &'static str) -> Result<Option<f64>, RepositoryError> {
    row.try_get::<Option<f64>, _>(column)
        .map_err(|_| RepositoryError::Backend(decode_failure(column)))
}

/// A uuid column selected with `::text`, parsed back into `FlexUuid`.
fn col_uuid(row: &PgRow, column: &'static str) -> Result<FlexUuid, RepositoryError> {
    let raw = col_str(row, column)?;
    uuid_from_text(&raw).map_err(|_| RepositoryError::Backend(decode_failure(column)))
}

/// A timestamptz column (UTC from the driver) as the contracts `EventTime`.
fn col_event_time(row: &PgRow, column: &'static str) -> Result<EventTime, RepositoryError> {
    let raw: OffsetDateTime = row
        .try_get(column)
        .map_err(|_| RepositoryError::Backend(decode_failure(column)))?;
    EventTime::new(raw).map_err(|_| RepositoryError::Backend(decode_failure(column)))
}

/// A descriptor jsonb column as the domain's reference string: a JSON
/// string round-trips exactly; any other jsonb shape renders as its JSON
/// text (rows written by this adapter always store the JSON-string form).
fn col_json_ref(row: &PgRow, column: &'static str) -> Result<String, RepositoryError> {
    let value: Value = row
        .try_get(column)
        .map_err(|_| RepositoryError::Backend(decode_failure(column)))?;
    Ok(match value {
        Value::String(text) => text,
        other => serde_json::to_string(&other)
            .unwrap_or_else(|_| "{}".to_string()),
    })
}

// --- Entity decoders (parent-chain reconstruction) ----------------------------

fn decode_tenant(row: &PgRow) -> Result<Tenant, RepositoryError> {
    let tenant_id = TenantId::new(col_uuid(row, "tenant_id")?);
    let name = col_str(row, "name")?;
    let description = col_opt_str(row, "description")?;
    let idp_reference = col_str(row, "idp_reference")?;
    let policy_profile = col_str(row, "policy_profile")?;
    let created_at = col_event_time(row, "created_at")?;
    Tenant::try_new(
        tenant_id,
        name,
        description,
        idp_reference,
        policy_profile,
        created_at,
    )
    .map_err(|_| RepositoryError::Backend(decode_failure("tenants row")))
}

fn decode_portfolio_with(row: &PgRow, tenant: &Tenant) -> Result<Portfolio, RepositoryError> {
    let portfolio_id = PortfolioId::new(col_uuid(row, "portfolio_id")?);
    let name = col_str(row, "name")?;
    let description = col_opt_str(row, "description")?;
    let operator = col_str(row, "operator")?;
    let market_region = col_str(row, "market_region")?;
    let aggregation_rules = col_json_ref(row, "aggregation_rules")?;
    let created_at = col_event_time(row, "created_at")?;
    Portfolio::try_new(
        tenant,
        portfolio_id,
        name,
        description,
        operator,
        market_region,
        aggregation_rules,
        created_at,
    )
    .map_err(|_| RepositoryError::Backend(decode_failure("portfolios row")))
}

fn decode_site_with(row: &PgRow, portfolio: &Portfolio) -> Result<Site, RepositoryError> {
    let site_id = SiteId::new(col_uuid(row, "site_id")?);
    let name = col_str(row, "name")?;
    let description = col_opt_str(row, "description")?;
    let location = col_json_ref(row, "location")?;
    let timezone = SiteTimezone::new(col_str(row, "timezone")?)
        .map_err(|_| RepositoryError::Backend(decode_failure("sites.timezone")))?;
    let pcc = col_json_ref(row, "pcc")?;
    // The domain entity requires non-empty grid/edge references; rows
    // written by this adapter always carry them (the columns allow NULL for
    // later-phase writers, which cannot round-trip the P1.06 entity).
    let grid_model_reference = col_str(row, "grid_model_reference")?;
    let edge_identity = col_str(row, "edge_identity")?;
    let created_at = col_event_time(row, "created_at")?;
    Site::try_new(
        portfolio,
        site_id,
        name,
        description,
        location,
        timezone,
        pcc,
        grid_model_reference,
        edge_identity,
        created_at,
    )
    .map_err(|_| RepositoryError::Backend(decode_failure("sites row")))
}

fn decode_asset_with(row: &PgRow, site: &Site) -> Result<Asset, RepositoryError> {
    let asset_id = AssetId::new(col_uuid(row, "asset_id")?);
    let name = col_str(row, "name")?;
    let description = col_opt_str(row, "description")?;
    let asset_type = asset_type_from_text(&col_str(row, "asset_type")?)?;
    let vendor_model = col_str(row, "vendor_model")?;
    let rated_power_mw = ActivePowerMW::new(col_f64(row, "rated_power_mw")?)
        .map_err(|_| RepositoryError::Backend(decode_failure("assets.rated_power_mw")))?;
    let rated_energy_mwh = match col_opt_f64(row, "rated_energy_mwh")? {
        Some(value) => Some(EnergyMWh::new(value).map_err(|_| {
            RepositoryError::Backend(decode_failure("assets.rated_energy_mwh"))
        })?),
        None => None,
    };
    let phase = col_str(row, "phase")?;
    let bus_reference = col_str(row, "bus_reference")?;
    let capability_profile = col_json_ref(row, "capability_profile")?;
    let created_at = col_event_time(row, "created_at")?;
    Asset::try_new(
        site,
        asset_id,
        name,
        description,
        asset_type,
        vendor_model,
        rated_power_mw,
        rated_energy_mwh,
        phase,
        bus_reference,
        capability_profile,
        created_at,
    )
    .map_err(|_| RepositoryError::Backend(decode_failure("assets row")))
}

// --- Parent-chain loaders (inside the caller's transaction) -------------------

async fn load_tenant_by_id(
    tx: &mut Tx,
    tenant_id_text: &str,
) -> Result<Tenant, RepositoryError> {
    let row = sqlx::query(&format!("{TENANT_SELECT} WHERE tenant_id = $1::uuid"))
        .bind(tenant_id_text)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|err| map_repository_error(entities::TENANTS, err))?
        .ok_or_else(|| RepositoryError::Backend(referential_failure("tenants")))?;
    decode_tenant(&row)
}

async fn load_portfolio_by_id(
    tx: &mut Tx,
    portfolio_id_text: &str,
) -> Result<Portfolio, RepositoryError> {
    let row = sqlx::query(&format!(
        "{PORTFOLIO_SELECT} WHERE portfolio_id = $1::uuid"
    ))
    .bind(portfolio_id_text)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|err| map_repository_error(entities::PORTFOLIOS, err))?
    .ok_or_else(|| RepositoryError::Backend(referential_failure("portfolios")))?;
    let tenant = load_tenant_by_id(&mut *tx, &col_str(&row, "tenant_id")?).await?;
    decode_portfolio_with(&row, &tenant)
}

async fn load_site_by_id(tx: &mut Tx, site_id_text: &str) -> Result<Site, RepositoryError> {
    let row = sqlx::query(&format!("{SITE_SELECT} WHERE site_id = $1::uuid"))
        .bind(site_id_text)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|err| map_repository_error(entities::SITES, err))?
        .ok_or_else(|| RepositoryError::Backend(referential_failure("sites")))?;
    let portfolio =
        load_portfolio_by_id(&mut *tx, &col_str(&row, "portfolio_id")?).await?;
    decode_site_with(&row, &portfolio)
}

// --- TenantRepository ---------------------------------------------------------

impl TenantRepository for PgUnitOfWork {
    fn insert(&self, tenant: &Tenant) -> Result<StateChangeReceipt, RepositoryError> {
        self.runtime.block_on(async {
            let mut guard = self.tx.lock().await;
            let tx = tx_ref(&mut guard).await.map_err(RepositoryError::Backend)?;
            sqlx::query(
                "INSERT INTO tenants \
                     (tenant_id, name, description, idp_reference, policy_profile, created_at, updated_at) \
                 VALUES ($1::uuid, $2, $3, $4, $5, $6, $6)",
            )
            .bind(uuid_text(tenant.tenant_id().uuid()))
            .bind(tenant.name())
            .bind(tenant.description())
            .bind(tenant.identity_provider_reference())
            .bind(tenant.policy_profile())
            .bind(tenant.created_at().timestamp())
            .execute(&mut *tx)
            .await
            .map_err(|err| map_repository_error(entities::TENANTS, err))?;
            Ok(StateChangeReceipt::new(
                entities::TENANTS.to_string(),
                StateChangeOperation::Insert,
                None,
            ))
        })
    }

    fn update(&self, tenant: &Tenant) -> Result<StateChangeReceipt, RepositoryError> {
        self.runtime.block_on(async {
            let mut guard = self.tx.lock().await;
            let tx = tx_ref(&mut guard).await.map_err(RepositoryError::Backend)?;
            let result = sqlx::query(
                "UPDATE tenants SET name = $2, description = $3, idp_reference = $4, \
                        policy_profile = $5, updated_at = now() \
                 WHERE tenant_id = $1::uuid",
            )
            .bind(uuid_text(tenant.tenant_id().uuid()))
            .bind(tenant.name())
            .bind(tenant.description())
            .bind(tenant.identity_provider_reference())
            .bind(tenant.policy_profile())
            .execute(&mut *tx)
            .await
            .map_err(|err| map_repository_error(entities::TENANTS, err))?;
            if result.rows_affected() == 0 {
                return Err(RepositoryError::NotFound {
                    entity: entities::TENANTS,
                });
            }
            Ok(StateChangeReceipt::new(
                entities::TENANTS.to_string(),
                StateChangeOperation::Update,
                None,
            ))
        })
    }

    fn find_by_id(&self, tenant_id: TenantId) -> Result<Option<Tenant>, RepositoryError> {
        let id_text = uuid_text(tenant_id.uuid());
        self.runtime.block_on(async {
            let mut guard = self.tx.lock().await;
            let tx = tx_ref(&mut guard).await.map_err(RepositoryError::Backend)?;
            let row = sqlx::query(&format!("{TENANT_SELECT} WHERE tenant_id = $1::uuid"))
                .bind(&id_text)
                .fetch_optional(&mut *tx)
                .await
                .map_err(|err| map_repository_error(entities::TENANTS, err))?;
            match row {
                None => Ok(None),
                Some(row) => Ok(Some(decode_tenant(&row)?)),
            }
        })
    }

    fn find_by_identity_provider_reference(
        &self,
        identity_provider_reference: &str,
    ) -> Result<Option<Tenant>, RepositoryError> {
        let reference = identity_provider_reference.to_string();
        self.runtime.block_on(async {
            let mut guard = self.tx.lock().await;
            let tx = tx_ref(&mut guard).await.map_err(RepositoryError::Backend)?;
            let row = sqlx::query(&format!(
                "{TENANT_SELECT} WHERE idp_reference = $1"
            ))
            .bind(&reference)
            .fetch_optional(&mut *tx)
            .await
            .map_err(|err| map_repository_error(entities::TENANTS, err))?;
            match row {
                None => Ok(None),
                Some(row) => Ok(Some(decode_tenant(&row)?)),
            }
        })
    }

    fn list_all(&self) -> Result<Vec<Tenant>, RepositoryError> {
        self.runtime.block_on(async {
            let mut guard = self.tx.lock().await;
            let tx = tx_ref(&mut guard).await.map_err(RepositoryError::Backend)?;
            let rows = sqlx::query(&format!(
                "{TENANT_SELECT} ORDER BY created_at, tenant_id"
            ))
            .fetch_all(&mut *tx)
            .await
            .map_err(|err| map_repository_error(entities::TENANTS, err))?;
            rows.iter().map(decode_tenant).collect()
        })
    }

    fn delete(&self, tenant_id: TenantId) -> Result<bool, RepositoryError> {
        let id_text = uuid_text(tenant_id.uuid());
        self.runtime.block_on(async {
            let mut guard = self.tx.lock().await;
            let tx = tx_ref(&mut guard).await.map_err(RepositoryError::Backend)?;
            let result = sqlx::query("DELETE FROM tenants WHERE tenant_id = $1::uuid")
                .bind(&id_text)
                .execute(&mut *tx)
                .await
                .map_err(|err| map_repository_error(entities::TENANTS, err))?;
            Ok(result.rows_affected() > 0)
        })
    }
}

// --- PortfolioRepository -------------------------------------------------------

fn require_own_tenant(
    ctx: &TenantContext,
    entity_tenant: TenantId,
    entity: &'static str,
) -> Result<(), RepositoryError> {
    if entity_tenant != ctx.tenant_id() {
        Err(RepositoryError::TenantMismatch { entity })
    } else {
        Ok(())
    }
}

impl PortfolioRepository for PgUnitOfWork {
    fn insert(
        &self,
        ctx: &TenantContext,
        portfolio: &Portfolio,
    ) -> Result<StateChangeReceipt, RepositoryError> {
        require_own_tenant(ctx, portfolio.tenant_id(), entities::PORTFOLIOS)?;
        let tenant_text = uuid_text(ctx.tenant_id().uuid());
        self.runtime.block_on(async {
            let mut guard = self.tx.lock().await;
            let tx = tx_ref(&mut guard).await.map_err(RepositoryError::Backend)?;
            sqlx::query(
                "INSERT INTO portfolios \
                     (portfolio_id, tenant_id, name, description, operator, market_region, \
                      aggregation_rules, created_at, updated_at) \
                 VALUES ($1::uuid, $2::uuid, $3, $4, $5, $6, $7, $8, $8)",
            )
            .bind(uuid_text(portfolio.portfolio_id().uuid()))
            .bind(&tenant_text)
            .bind(portfolio.name())
            .bind(portfolio.description())
            .bind(portfolio.operator())
            .bind(portfolio.market_region())
            .bind(Value::String(portfolio.aggregation_rules().to_string()))
            .bind(portfolio.created_at().timestamp())
            .execute(&mut *tx)
            .await
            .map_err(|err| map_repository_error(entities::PORTFOLIOS, err))?;
            Ok(StateChangeReceipt::new(
                entities::PORTFOLIOS.to_string(),
                StateChangeOperation::Insert,
                Some(ctx.tenant_id()),
            ))
        })
    }

    fn update(
        &self,
        ctx: &TenantContext,
        portfolio: &Portfolio,
    ) -> Result<StateChangeReceipt, RepositoryError> {
        require_own_tenant(ctx, portfolio.tenant_id(), entities::PORTFOLIOS)?;
        let tenant_text = uuid_text(ctx.tenant_id().uuid());
        self.runtime.block_on(async {
            let mut guard = self.tx.lock().await;
            let tx = tx_ref(&mut guard).await.map_err(RepositoryError::Backend)?;
            let result = sqlx::query(
                "UPDATE portfolios SET name = $3, description = $4, operator = $5, \
                        market_region = $6, aggregation_rules = $7, updated_at = now() \
                 WHERE portfolio_id = $1::uuid AND tenant_id = $2::uuid",
            )
            .bind(uuid_text(portfolio.portfolio_id().uuid()))
            .bind(&tenant_text)
            .bind(portfolio.name())
            .bind(portfolio.description())
            .bind(portfolio.operator())
            .bind(portfolio.market_region())
            .bind(Value::String(portfolio.aggregation_rules().to_string()))
            .execute(&mut *tx)
            .await
            .map_err(|err| map_repository_error(entities::PORTFOLIOS, err))?;
            if result.rows_affected() == 0 {
                return Err(RepositoryError::NotFound {
                    entity: entities::PORTFOLIOS,
                });
            }
            Ok(StateChangeReceipt::new(
                entities::PORTFOLIOS.to_string(),
                StateChangeOperation::Update,
                Some(ctx.tenant_id()),
            ))
        })
    }

    fn find_by_id(
        &self,
        ctx: &TenantContext,
        portfolio_id: PortfolioId,
    ) -> Result<Option<Portfolio>, RepositoryError> {
        let id_text = uuid_text(portfolio_id.uuid());
        let tenant_text = uuid_text(ctx.tenant_id().uuid());
        self.runtime.block_on(async {
            let mut guard = self.tx.lock().await;
            let tx = tx_ref(&mut guard).await.map_err(RepositoryError::Backend)?;
            let row = sqlx::query(&format!(
                "{PORTFOLIO_SELECT} WHERE portfolio_id = $1::uuid AND tenant_id = $2::uuid"
            ))
            .bind(&id_text)
            .bind(&tenant_text)
            .fetch_optional(&mut *tx)
            .await
            .map_err(|err| map_repository_error(entities::PORTFOLIOS, err))?;
            match row {
                // A foreign-tenant row reads as None — no existence leak
                // (P1.07 port contract; RLS already filtered it).
                None => Ok(None),
                Some(row) => {
                    let tenant =
                        load_tenant_by_id(&mut *tx, &col_str(&row, "tenant_id")?).await?;
                    Ok(Some(decode_portfolio_with(&row, &tenant)?))
                }
            }
        })
    }

    fn list_for_tenant(&self, ctx: &TenantContext) -> Result<Vec<Portfolio>, RepositoryError> {
        let tenant_text = uuid_text(ctx.tenant_id().uuid());
        self.runtime.block_on(async {
            let mut guard = self.tx.lock().await;
            let tx = tx_ref(&mut guard).await.map_err(RepositoryError::Backend)?;
            let rows = sqlx::query(&format!(
                "{PORTFOLIO_SELECT} WHERE tenant_id = $1::uuid ORDER BY created_at, portfolio_id"
            ))
            .bind(&tenant_text)
            .fetch_all(&mut *tx)
            .await
            .map_err(|err| map_repository_error(entities::PORTFOLIOS, err))?;
            let mut portfolios = Vec::with_capacity(rows.len());
            for row in &rows {
                let tenant = load_tenant_by_id(&mut *tx, &col_str(row, "tenant_id")?).await?;
                portfolios.push(decode_portfolio_with(row, &tenant)?);
            }
            Ok(portfolios)
        })
    }

    fn delete(
        &self,
        ctx: &TenantContext,
        portfolio_id: PortfolioId,
    ) -> Result<bool, RepositoryError> {
        let id_text = uuid_text(portfolio_id.uuid());
        let tenant_text = uuid_text(ctx.tenant_id().uuid());
        self.runtime.block_on(async {
            let mut guard = self.tx.lock().await;
            let tx = tx_ref(&mut guard).await.map_err(RepositoryError::Backend)?;
            let result = sqlx::query(
                "DELETE FROM portfolios WHERE portfolio_id = $1::uuid AND tenant_id = $2::uuid",
            )
            .bind(&id_text)
            .bind(&tenant_text)
            .execute(&mut *tx)
            .await
            .map_err(|err| map_repository_error(entities::PORTFOLIOS, err))?;
            Ok(result.rows_affected() > 0)
        })
    }
}

// --- SiteRepository ------------------------------------------------------------

impl SiteRepository for PgUnitOfWork {
    fn insert(
        &self,
        ctx: &TenantContext,
        site: &Site,
    ) -> Result<StateChangeReceipt, RepositoryError> {
        require_own_tenant(ctx, site.tenant_id(), entities::SITES)?;
        let tenant_text = uuid_text(ctx.tenant_id().uuid());
        self.runtime.block_on(async {
            let mut guard = self.tx.lock().await;
            let tx = tx_ref(&mut guard).await.map_err(RepositoryError::Backend)?;
            sqlx::query(
                "INSERT INTO sites \
                     (site_id, portfolio_id, tenant_id, name, description, location, timezone, \
                      pcc, grid_model_reference, edge_identity, created_at, updated_at) \
                 VALUES ($1::uuid, $2::uuid, $3::uuid, $4, $5, $6, $7, $8, $9, $10, $11, $11)",
            )
            .bind(uuid_text(site.site_id().uuid()))
            .bind(uuid_text(site.portfolio_id().uuid()))
            .bind(&tenant_text)
            .bind(site.name())
            .bind(site.description())
            .bind(Value::String(site.location().to_string()))
            .bind(site.timezone().as_str())
            .bind(Value::String(site.point_of_common_coupling().to_string()))
            .bind(site.grid_model_reference())
            .bind(site.edge_identity())
            .bind(site.created_at().timestamp())
            .execute(&mut *tx)
            .await
            .map_err(|err| map_repository_error(entities::SITES, err))?;
            Ok(StateChangeReceipt::new(
                entities::SITES.to_string(),
                StateChangeOperation::Insert,
                Some(ctx.tenant_id()),
            ))
        })
    }

    fn update(
        &self,
        ctx: &TenantContext,
        site: &Site,
    ) -> Result<StateChangeReceipt, RepositoryError> {
        require_own_tenant(ctx, site.tenant_id(), entities::SITES)?;
        let tenant_text = uuid_text(ctx.tenant_id().uuid());
        self.runtime.block_on(async {
            let mut guard = self.tx.lock().await;
            let tx = tx_ref(&mut guard).await.map_err(RepositoryError::Backend)?;
            let result = sqlx::query(
                "UPDATE sites SET name = $3, description = $4, location = $5, timezone = $6, \
                        pcc = $7, grid_model_reference = $8, edge_identity = $9, updated_at = now() \
                 WHERE site_id = $1::uuid AND tenant_id = $2::uuid",
            )
            .bind(uuid_text(site.site_id().uuid()))
            .bind(&tenant_text)
            .bind(site.name())
            .bind(site.description())
            .bind(Value::String(site.location().to_string()))
            .bind(site.timezone().as_str())
            .bind(Value::String(site.point_of_common_coupling().to_string()))
            .bind(site.grid_model_reference())
            .bind(site.edge_identity())
            .execute(&mut *tx)
            .await
            .map_err(|err| map_repository_error(entities::SITES, err))?;
            if result.rows_affected() == 0 {
                return Err(RepositoryError::NotFound {
                    entity: entities::SITES,
                });
            }
            Ok(StateChangeReceipt::new(
                entities::SITES.to_string(),
                StateChangeOperation::Update,
                Some(ctx.tenant_id()),
            ))
        })
    }

    fn find_by_id(
        &self,
        ctx: &TenantContext,
        site_id: SiteId,
    ) -> Result<Option<Site>, RepositoryError> {
        let id_text = uuid_text(site_id.uuid());
        let tenant_text = uuid_text(ctx.tenant_id().uuid());
        self.runtime.block_on(async {
            let mut guard = self.tx.lock().await;
            let tx = tx_ref(&mut guard).await.map_err(RepositoryError::Backend)?;
            let row = sqlx::query(&format!(
                "{SITE_SELECT} WHERE site_id = $1::uuid AND tenant_id = $2::uuid"
            ))
            .bind(&id_text)
            .bind(&tenant_text)
            .fetch_optional(&mut *tx)
            .await
            .map_err(|err| map_repository_error(entities::SITES, err))?;
            match row {
                None => Ok(None),
                Some(row) => {
                    let portfolio =
                        load_portfolio_by_id(&mut *tx, &col_str(&row, "portfolio_id")?).await?;
                    Ok(Some(decode_site_with(&row, &portfolio)?))
                }
            }
        })
    }

    fn list_for_tenant(&self, ctx: &TenantContext) -> Result<Vec<Site>, RepositoryError> {
        let tenant_text = uuid_text(ctx.tenant_id().uuid());
        self.runtime.block_on(async {
            let mut guard = self.tx.lock().await;
            let tx = tx_ref(&mut guard).await.map_err(RepositoryError::Backend)?;
            let rows = sqlx::query(&format!(
                "{SITE_SELECT} WHERE tenant_id = $1::uuid ORDER BY created_at, site_id"
            ))
            .bind(&tenant_text)
            .fetch_all(&mut *tx)
            .await
            .map_err(|err| map_repository_error(entities::SITES, err))?;
            let mut sites = Vec::with_capacity(rows.len());
            for row in &rows {
                let portfolio =
                    load_portfolio_by_id(&mut *tx, &col_str(row, "portfolio_id")?).await?;
                sites.push(decode_site_with(row, &portfolio)?);
            }
            Ok(sites)
        })
    }

    fn delete(&self, ctx: &TenantContext, site_id: SiteId) -> Result<bool, RepositoryError> {
        let id_text = uuid_text(site_id.uuid());
        let tenant_text = uuid_text(ctx.tenant_id().uuid());
        self.runtime.block_on(async {
            let mut guard = self.tx.lock().await;
            let tx = tx_ref(&mut guard).await.map_err(RepositoryError::Backend)?;
            let result = sqlx::query(
                "DELETE FROM sites WHERE site_id = $1::uuid AND tenant_id = $2::uuid",
            )
            .bind(&id_text)
            .bind(&tenant_text)
            .execute(&mut *tx)
            .await
            .map_err(|err| map_repository_error(entities::SITES, err))?;
            Ok(result.rows_affected() > 0)
        })
    }
}

// --- AssetRepository ------------------------------------------------------------
//
// Insert also writes the initial asset_state row (D-S2, P1.12 §4.5 Phase 1
// write convention): event_time = ingest_time = registration time, quality
// = 'MISSING', NULL measurements — in the same transaction as the asset.

impl AssetRepository for PgUnitOfWork {
    fn insert(
        &self,
        ctx: &TenantContext,
        asset: &Asset,
    ) -> Result<StateChangeReceipt, RepositoryError> {
        require_own_tenant(ctx, asset.tenant_id(), entities::ASSETS)?;
        let tenant_text = uuid_text(ctx.tenant_id().uuid());
        self.runtime.block_on(async {
            let mut guard = self.tx.lock().await;
            let tx = tx_ref(&mut guard).await.map_err(RepositoryError::Backend)?;
            sqlx::query(
                "INSERT INTO assets \
                     (asset_id, site_id, tenant_id, name, description, asset_type, vendor_model, \
                      rated_power_mw, rated_energy_mwh, phase, bus_reference, capability_profile, \
                      created_at, updated_at) \
                 VALUES ($1::uuid, $2::uuid, $3::uuid, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $13)",
            )
            .bind(uuid_text(asset.asset_id().uuid()))
            .bind(uuid_text(asset.site_id().uuid()))
            .bind(&tenant_text)
            .bind(asset.name())
            .bind(asset.description())
            .bind(asset_type_text(asset.asset_type()))
            .bind(asset.vendor_model())
            .bind(asset.rated_power_mw().value())
            .bind(asset.rated_energy_mwh().map(|energy| energy.value()))
            .bind(asset.phase())
            .bind(asset.bus_reference())
            .bind(Value::String(asset.capability_profile().to_string()))
            .bind(asset.created_at().timestamp())
            .execute(&mut *tx)
            .await
            .map_err(|err| map_repository_error(entities::ASSETS, err))?;
            // Initial current-state row (P1.12 §4.5 Phase 1 write convention).
            sqlx::query(
                "INSERT INTO asset_state (asset_id, tenant_id, event_time, ingest_time, quality) \
                 VALUES ($1::uuid, $2::uuid, now(), now(), 'MISSING')",
            )
            .bind(uuid_text(asset.asset_id().uuid()))
            .bind(&tenant_text)
            .execute(&mut *tx)
            .await
            .map_err(|err| map_repository_error(entities::ASSETS, err))?;
            Ok(StateChangeReceipt::new(
                entities::ASSETS.to_string(),
                StateChangeOperation::Insert,
                Some(ctx.tenant_id()),
            ))
        })
    }

    fn update(
        &self,
        ctx: &TenantContext,
        asset: &Asset,
    ) -> Result<StateChangeReceipt, RepositoryError> {
        require_own_tenant(ctx, asset.tenant_id(), entities::ASSETS)?;
        let tenant_text = uuid_text(ctx.tenant_id().uuid());
        self.runtime.block_on(async {
            let mut guard = self.tx.lock().await;
            let tx = tx_ref(&mut guard).await.map_err(RepositoryError::Backend)?;
            let result = sqlx::query(
                "UPDATE assets SET name = $3, description = $4, asset_type = $5, \
                        vendor_model = $6, rated_power_mw = $7, rated_energy_mwh = $8, \
                        phase = $9, bus_reference = $10, capability_profile = $11, updated_at = now() \
                 WHERE asset_id = $1::uuid AND tenant_id = $2::uuid",
            )
            .bind(uuid_text(asset.asset_id().uuid()))
            .bind(&tenant_text)
            .bind(asset.name())
            .bind(asset.description())
            .bind(asset_type_text(asset.asset_type()))
            .bind(asset.vendor_model())
            .bind(asset.rated_power_mw().value())
            .bind(asset.rated_energy_mwh().map(|energy| energy.value()))
            .bind(asset.phase())
            .bind(asset.bus_reference())
            .bind(Value::String(asset.capability_profile().to_string()))
            .execute(&mut *tx)
            .await
            .map_err(|err| map_repository_error(entities::ASSETS, err))?;
            if result.rows_affected() == 0 {
                return Err(RepositoryError::NotFound {
                    entity: entities::ASSETS,
                });
            }
            Ok(StateChangeReceipt::new(
                entities::ASSETS.to_string(),
                StateChangeOperation::Update,
                Some(ctx.tenant_id()),
            ))
        })
    }

    fn find_by_id(
        &self,
        ctx: &TenantContext,
        asset_id: AssetId,
    ) -> Result<Option<Asset>, RepositoryError> {
        let id_text = uuid_text(asset_id.uuid());
        let tenant_text = uuid_text(ctx.tenant_id().uuid());
        self.runtime.block_on(async {
            let mut guard = self.tx.lock().await;
            let tx = tx_ref(&mut guard).await.map_err(RepositoryError::Backend)?;
            let row = sqlx::query(&format!(
                "{ASSET_SELECT} WHERE asset_id = $1::uuid AND tenant_id = $2::uuid"
            ))
            .bind(&id_text)
            .bind(&tenant_text)
            .fetch_optional(&mut *tx)
            .await
            .map_err(|err| map_repository_error(entities::ASSETS, err))?;
            match row {
                None => Ok(None),
                Some(row) => {
                    let site = load_site_by_id(&mut *tx, &col_str(&row, "site_id")?).await?;
                    Ok(Some(decode_asset_with(&row, &site)?))
                }
            }
        })
    }

    fn list_for_tenant(&self, ctx: &TenantContext) -> Result<Vec<Asset>, RepositoryError> {
        let tenant_text = uuid_text(ctx.tenant_id().uuid());
        self.runtime.block_on(async {
            let mut guard = self.tx.lock().await;
            let tx = tx_ref(&mut guard).await.map_err(RepositoryError::Backend)?;
            let rows = sqlx::query(&format!(
                "{ASSET_SELECT} WHERE tenant_id = $1::uuid ORDER BY created_at, asset_id"
            ))
            .bind(&tenant_text)
            .fetch_all(&mut *tx)
            .await
            .map_err(|err| map_repository_error(entities::ASSETS, err))?;
            let mut assets = Vec::with_capacity(rows.len());
            for row in &rows {
                let site = load_site_by_id(&mut *tx, &col_str(row, "site_id")?).await?;
                assets.push(decode_asset_with(row, &site)?);
            }
            Ok(assets)
        })
    }

    fn delete(&self, ctx: &TenantContext, asset_id: AssetId) -> Result<bool, RepositoryError> {
        let id_text = uuid_text(asset_id.uuid());
        let tenant_text = uuid_text(ctx.tenant_id().uuid());
        self.runtime.block_on(async {
            let mut guard = self.tx.lock().await;
            let tx = tx_ref(&mut guard).await.map_err(RepositoryError::Backend)?;
            let result = sqlx::query(
                "DELETE FROM assets WHERE asset_id = $1::uuid AND tenant_id = $2::uuid",
            )
            .bind(&id_text)
            .bind(&tenant_text)
            .execute(&mut *tx)
            .await
            .map_err(|err| map_repository_error(entities::ASSETS, err))?;
            Ok(result.rows_affected() > 0)
        })
    }
}

// --- EventOutbox (unit-of-work side: the transactional append) ------------------

impl EventOutbox for PgUnitOfWork {
    /// Appends the outbox row inside THIS unit of work's native transaction.
    /// The state-change half of `write` was written by a repository call on
    /// this same unit of work (the receipt it returned is consumed by
    /// `OutboxWrite::new`), so both halves are committed or rolled back
    /// together — the §6.3 step 1 atomicity rule holds by construction.
    fn append(&self, write: OutboxWrite) -> Result<(), EventOutboxError> {
        let (event_type, schema_version) =
            outbox::outbox_classifiers(write.payload()).map_err(EventOutboxError::Backend)?;
        let payload = outbox::outbox_payload_json(write.payload())?;
        let key = write.idempotency_key().to_string();
        let subject = write.destination_subject().to_string();
        let created_at = write.created_at().timestamp();
        self.runtime.block_on(async {
            let mut guard = self.tx.lock().await;
            let tx = tx_ref(&mut guard).await.map_err(EventOutboxError::Backend)?;
            outbox::insert_outbox_row(
                &mut *tx,
                &key,
                &event_type,
                &schema_version,
                payload,
                &subject,
                created_at,
            )
            .await
        })
    }

    /// Pending entries read through the transaction (sees the unit of
    /// work's own uncommitted appends; the publisher normally reads via
    /// [`crate::FlexCoreStore`]).
    fn pending(&self, limit: u32) -> Result<Vec<PendingOutboxEntry>, EventOutboxError> {
        self.runtime.block_on(async {
            let mut guard = self.tx.lock().await;
            let tx = tx_ref(&mut guard).await.map_err(EventOutboxError::Backend)?;
            outbox::pending_entries(&mut *tx, limit).await
        })
    }

    fn record_attempt(&self, idempotency_key: &str) -> Result<(), EventOutboxError> {
        let key = idempotency_key.to_string();
        self.runtime.block_on(async {
            let mut guard = self.tx.lock().await;
            let tx = tx_ref(&mut guard).await.map_err(EventOutboxError::Backend)?;
            outbox::record_attempt(&mut *tx, &key).await
        })
    }

    fn mark_delivered(&self, idempotency_key: &str) -> Result<(), EventOutboxError> {
        let key = idempotency_key.to_string();
        self.runtime.block_on(async {
            let mut guard = self.tx.lock().await;
            let tx = tx_ref(&mut guard).await.map_err(EventOutboxError::Backend)?;
            outbox::mark_delivered(&mut *tx, &key).await
        })
    }

    fn mark_failed(&self, idempotency_key: &str) -> Result<(), EventOutboxError> {
        let key = idempotency_key.to_string();
        self.runtime.block_on(async {
            let mut guard = self.tx.lock().await;
            let tx = tx_ref(&mut guard).await.map_err(EventOutboxError::Backend)?;
            outbox::mark_failed(&mut *tx, &key).await
        })
    }
}

// --- AuditSink (unit-of-work side: append inside the transaction) ---------------

impl AuditSink for PgUnitOfWork {
    /// Appends one audit record inside THIS unit of work's native
    /// transaction, so evidence about a state change commits atomically
    /// with it when the use case needs that. The unit of work's tenant
    /// context governs the RLS check: a record for a different tenant is
    /// rejected, and platform-level records (`tenant_id IS NULL`) are only
    /// writable through the owner/ops path (P1.12 §5.2). The port exposes
    /// append only — no update, no delete (P1.10; the database trigger is
    /// layer 2 of the append-only enforcement, this port layer 3).
    fn append(&self, record: AuditRecord) -> Result<(), AuditError> {
        self.runtime.block_on(async {
            let mut guard = self.tx.lock().await;
            let tx = tx_ref(&mut guard).await.map_err(|backend| AuditError::Rejected {
                reason: backend.message().to_string(),
            })?;
            crate::audit::insert_audit_record(&mut *tx, &record).await
        })
    }
}
