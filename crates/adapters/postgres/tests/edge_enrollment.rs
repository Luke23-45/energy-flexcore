// P6.06 — Integration tests for the edge-enrollment persistence wiring
//         (migration 0018; report §16.1–§16.2, §17.3, §18.3; ledger
//         P6.06; decision record `06_07_identity_channel.md`).
//
// Covered on real PostgreSQL (gated on FLEXCORE_TEST_DATABASE_URL, the
// recorded harness decision — every test skips cleanly when unset):
//
// - **Enrollment persistence**: the device-identity registry row (migration
//   0018) and its paired ENROLLED journal event commit through the
//   `EdgeEnrollmentStore` port and read back through the tenant-scoped
//   methods (current term = highest `enrollment_seq`).
// - **Identity uniqueness (ledger acceptance: no two edges share an
//   identity)**: a second ACTIVE term for the same (device_id, tenant_id)
//   is the named `Conflict`; rotation after an EXPIRED term inserts
//   legally (the partial unique index admits exactly one ACTIVE term).
// - **Binding check**: `site_is_approved` is tenant-scoped — a site from
//   another tenant is NOT approved in this scope (no existence leak,
//   P0.16 rule 5).
// - **Append-only + tenant isolation (§16.2)**: UPDATE and DELETE are
//   refused by the migration 0018 guard triggers (even for the owner), and
//   a foreign-tenant device reads as `None`.
// - **Journal honesty**: a REJECTED event for a device that was never
//   enrolled is recordable (the journal carries no registry FK — the
//   documented migration choice) and carries a reason.
//
// UNRUN — written, never executed in this loop (the ENTRY_NOTE bans); the
// human/CI run with a PostgreSQL 18 service executes them.
//
// Roles: the runtime connects as the configured URL's role; the migration
// runner needs the owner role (`flexcore_migrator`) or a bootstrap
// administrator, per the P1.13 conventions.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

use flexcore_contracts::ids::FlexUuid;
use flexcore_contracts::time_model::{EventTime, SiteTimezone};
use flexcore_domain::entities::{Portfolio, Site, Tenant};
use flexcore_domain::ids::{PortfolioId, SiteId, TenantId};
use flexcore_domain::tenant_context::{AuthenticatedClaims, TenantContext, VerifiedClaims};
use flexcore_postgres_adapter::{FlexCoreStore, PgUnitOfWork};
use flexcore_storage::edge_enrollment::{
    EdgeDeviceIdentityRow, EdgeEnrollmentEventRow, EdgeEnrollmentStore as _,
    EdgeEnrollmentStoreError,
};
use flexcore_ports::repository::{PortfolioRepository as _, SiteRepository as _, TenantRepository as _};

// --- Harness helpers (the recorded P2.06/P3.21/P4.10 decisions) ---------------

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
                "skipped {test}: FLEXCORE_TEST_DATABASE_URL is not set (CI provides a PostgreSQL 18 service)"
            );
            None
        }
    }
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

fn next_counter() -> u64 {
    static COUNTER: AtomicU64 = AtomicU64::new(1);
    COUNTER.fetch_add(1, Ordering::SeqCst)
}

fn fixture_uuid(prefix: u8) -> FlexUuid {
    let mut bytes = [0u8; 16];
    bytes[0] = prefix;
    bytes[1] = 0xC6; // P6.06 fixture marker
    bytes[2] = (run_key() >> 56) as u8;
    bytes[3] = (run_key() >> 48) as u8;
    bytes[4] = (run_key() >> 40) as u8;
    bytes[5] = (run_key() >> 32) as u8;
    bytes[6] = (run_key() >> 24) as u8;
    bytes[7] = (run_key() >> 16) as u8;
    bytes[8] = (run_key() >> 8) as u8;
    bytes[9] = run_key() as u8;
    let counter = next_counter().to_be_bytes();
    bytes[10..16].copy_from_slice(&counter[2..8]);
    FlexUuid::from_bytes(bytes)
}

fn tenant_context(uuid: FlexUuid) -> TenantContext {
    TenantContext::from_authenticated_claims(AuthenticatedClaims::from_verified(
        VerifiedClaims {
            tenant_id: TenantId::new(uuid),
            subject: "p6.06-edge-enrollment-test".to_string(),
        },
    ))
}

/// Seeds tenant + portfolio + site (the registry's FK chain) in one
/// committed unit of work.
fn seed_environment(
    store: &FlexCoreStore,
    tenant_uuid: FlexUuid,
) -> (TenantContext, FlexUuid, FlexUuid) {
    let ctx = tenant_context(tenant_uuid);
    let portfolio_uuid = fixture_uuid(0xA2);
    let site_uuid = fixture_uuid(0xA3);
    let created = EventTime::from_unix_millis(1_700_000_000_000).expect("valid");
    let unit: PgUnitOfWork = store.begin(Some(&ctx)).expect("unit of work opens");
    let tenant = Tenant::try_new(
        TenantId::new(tenant_uuid),
        "p6.06 tenant".to_string(),
        None,
        format!("idp-{tenant_uuid}"),
        "default".to_string(),
        created,
    )
    .expect("tenant builds");
    flexcore_ports::repository::TenantRepository::insert(&unit, &tenant).expect("tenant inserts");
    let portfolio = Portfolio::try_new(
        &tenant,
        PortfolioId::new(portfolio_uuid),
        "p6.06 portfolio".to_string(),
        None,
        "operator".to_string(),
        "none (D1 OPEN)".to_string(),
        "{}".to_string(),
        created,
    )
    .expect("portfolio builds");
    flexcore_ports::repository::PortfolioRepository::insert(&unit, &ctx, &portfolio)
        .expect("portfolio inserts");
    let site = Site::try_new(
        &portfolio,
        SiteId::new(site_uuid),
        "p6.06 site".to_string(),
        None,
        "synthetic".to_string(),
        SiteTimezone::new("UTC".to_string()).expect("UTC"),
        "pcc-fixture".to_string(),
        "site-grid-model-reference-v1".to_string(),
        "edge-fixture".to_string(),
        created,
    )
    .expect("site builds");
    flexcore_ports::repository::SiteRepository::insert(&unit, &ctx, &site).expect("site inserts");
    unit.commit().expect("committed");
    (ctx, portfolio_uuid, site_uuid)
}

fn device_row(
    ctx: &TenantContext,
    device_id: &str,
    site_uuid: FlexUuid,
    seq: u64,
    status: &str,
) -> EdgeDeviceIdentityRow {
    EdgeDeviceIdentityRow::new(
        ctx.tenant_id(),
        device_id.to_string(),
        site_uuid.to_string(),
        seq,
        status.to_string(),
        Some("site-asset-set-v1".to_string()),
        1_700_000_000_000,
        Some(1_700_000_000_000 + 30 * 86_400_000),
    )
    .expect("the row builds")
}

fn journal_event(
    ctx: &TenantContext,
    device_id: &str,
    event_type: &str,
    reason: Option<&str>,
) -> EdgeEnrollmentEventRow {
    EdgeEnrollmentEventRow::new(
        ctx.tenant_id(),
        fixture_uuid(0xE1).to_string(),
        device_id.to_string(),
        event_type.to_string(),
        format!("edge:enrollment:{device_id}"),
        reason.map(|r| r.to_string()),
        format!("corr-{}", fixture_uuid(0xE2)),
        1_700_000_000_000,
    )
    .expect("the event row builds")
}

// --- Tests ---------------------------------------------------------------------

#[test]
fn an_enrollment_persists_and_reads_back_within_tenant_scope() {
    let Some(url) = require_db("an_enrollment_persists_and_reads_back_within_tenant_scope") else {
        return;
    };
    let store = FlexCoreStore::connect(&url).expect("connects");
    store.run_migrations().expect("migrations apply");
    let tenant_uuid = fixture_uuid(0xB1);
    let (ctx, _portfolio, site) = seed_environment(&store, tenant_uuid);

    store
        .enroll_device(&ctx, &device_row(&ctx, "edge-01", site, 1, "ACTIVE"))
        .expect("the identity term persists");
    store
        .append_event(&ctx, &journal_event(&ctx, "edge-01", "ENROLLED", None))
        .expect("the journal event persists");

    let current = store
        .find_device(&ctx, "edge-01")
        .expect("reads")
        .expect("the device exists in scope");
    assert_eq!(current.device_id, "edge-01");
    assert_eq!(current.status, "ACTIVE");
    assert_eq!(current.enrollment_seq, 1);
    assert_eq!(current.site_id, site.to_string());

    let events = store
        .list_events(&ctx, "edge-01", 100)
        .expect("the journal reads");
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].event_type, "ENROLLED");

    // Site approval is tenant-scoped: the site IS approved in its own scope.
    assert!(store.site_is_approved(&ctx, &site.to_string()).expect("reads"));
}

#[test]
fn a_duplicate_active_identity_is_the_named_conflict_and_rotation_inserts_legally() {
    let Some(url) = require_db("a_duplicate_active_identity_is_the_named_conflict_and_rotation_inserts_legally")
    else {
        return;
    };
    let store = FlexCoreStore::connect(&url).expect("connects");
    store.run_migrations().expect("migrations apply");
    let tenant_uuid = fixture_uuid(0xB2);
    let (ctx, _portfolio, site) = seed_environment(&store, tenant_uuid);

    store
        .enroll_device(&ctx, &device_row(&ctx, "edge-dup", site, 1, "ACTIVE"))
        .expect("the first ACTIVE term inserts");
    // A SECOND ACTIVE term for the same device identity = the Conflict (no
    // two edges share an identity; the partial unique index enforces it).
    let error = store
        .enroll_device(&ctx, &device_row(&ctx, "edge-dup", site, 2, "ACTIVE"))
        .expect_err("a duplicate ACTIVE identity must fail");
    assert!(
        matches!(error, EdgeEnrollmentStoreError::Conflict { ref device_id } if device_id == "edge-dup"),
        "the duplicate is the named Conflict: {error}"
    );

    // Rotation: the latest term flips to EXPIRED (a NEW row), and a NEW
    // ACTIVE term at the next sequence inserts legally.
    store
        .enroll_device(&ctx, &device_row(&ctx, "edge-dup", site, 2, "EXPIRED"))
        .expect("the EXPIRED term row inserts (append-only lifecycle)");
    let error_expired_seq = store
        .enroll_device(&ctx, &device_row(&ctx, "edge-dup", site, 2, "EXPIRED"))
        .expect_err("a duplicate sequence is the primary-key conflict");
    assert!(matches!(error_expired_seq, EdgeEnrollmentStoreError::Conflict { .. }));
    store
        .enroll_device(&ctx, &device_row(&ctx, "edge-dup", site, 3, "ACTIVE"))
        .expect("rotation to a new ACTIVE term inserts legally");
    let current = store
        .find_device(&ctx, "edge-dup")
        .expect("reads")
        .expect("present");
    assert_eq!(current.enrollment_seq, 3, "the CURRENT term is the highest sequence");
    assert_eq!(current.status, "ACTIVE");
}

#[test]
fn a_foreign_tenant_device_reads_as_none_and_leaks_nothing() {
    let Some(url) = require_db("a_foreign_tenant_device_reads_as_none_and_leaks_nothing") else {
        return;
    };
    let store = FlexCoreStore::connect(&url).expect("connects");
    store.run_migrations().expect("migrations apply");
    let tenant_a = fixture_uuid(0xB3);
    let tenant_b = fixture_uuid(0xB4);
    let (ctx_a, _portfolio_a, site_a) = seed_environment(&store, tenant_a);
    let (ctx_b, _portfolio_b, site_b) = seed_environment(&store, tenant_b);

    store
        .enroll_device(&ctx_a, &device_row(&ctx_a, "edge-cross", site_a, 1, "ACTIVE"))
        .expect("tenant A enrolls");

    // Tenant B cannot see the device (None — no existence leak), cannot
    // claim tenant A's site as approved, and tenant B's OWN site is not
    // approved in tenant A's scope.
    let from_b = store.find_device(&ctx_b, "edge-cross").expect("reads");
    assert_eq!(from_b, None, "a foreign-tenant device reads as None");
    assert!(
        !store
            .site_is_approved(&ctx_b, &site_a.to_string())
            .expect("reads"),
        "tenant A's site is not approved in tenant B's scope"
    );
    assert!(
        !store
            .site_is_approved(&ctx_a, &site_b.to_string())
            .expect("reads"),
        "tenant B's site is not approved in tenant A's scope"
    );
}

#[test]
fn the_registry_and_journal_refuse_update_and_delete() {
    let Some(url) = require_db("the_registry_and_journal_refuse_update_and_delete") else {
        return;
    };
    let store = FlexCoreStore::connect(&url).expect("connects");
    store.run_migrations().expect("migrations apply");
    let tenant_uuid = fixture_uuid(0xB5);
    let (ctx, _portfolio, site) = seed_environment(&store, tenant_uuid);
    store
        .enroll_device(&ctx, &device_row(&ctx, "edge-immutable", site, 1, "ACTIVE"))
        .expect("enrolls");
    store
        .append_event(&ctx, &journal_event(&ctx, "edge-immutable", "ENROLLED", None))
        .expect("journal event");

    // Raw UPDATE/DELETE (even as the OWNER — the trigger refuses
    // unconditionally; the 0008 pattern).
    let runtime = tokio::runtime::Runtime::new().expect("runtime");
    for statement in [
        "UPDATE edge_device_registry SET status = 'REVOKED' WHERE device_id = 'edge-immutable'",
        "DELETE FROM edge_device_registry WHERE device_id = 'edge-immutable'",
        "DELETE FROM edge_enrollment_journal WHERE device_id = 'edge-immutable'",
    ] {
        let result = runtime.block_on(async {
            sqlx::query(statement).execute(store.pool()).await
        });
        assert!(
            result.is_err(),
            "append-only violation must be refused: {statement}"
        );
    }

    // The rows survive every refused mutation.
    let current = store
        .find_device(&ctx, "edge-immutable")
        .expect("reads")
        .expect("present");
    assert_eq!(current.status, "ACTIVE");
}

#[test]
fn a_rejected_journal_event_for_a_never_enrolled_device_is_recordable() {
    let Some(url) = require_db("a_rejected_journal_event_for_a_never_enrolled_device_is_recordable")
    else {
        return;
    };
    let store = FlexCoreStore::connect(&url).expect("connects");
    store.run_migrations().expect("migrations apply");
    let tenant_uuid = fixture_uuid(0xB6);
    let (ctx, _portfolio, _site) = seed_environment(&store, tenant_uuid);

    // A REJECTED event names a device that never enrolled: the journal has
    // NO registry FK (the documented migration choice) — the §17.3 audit
    // trail records the refusal with its reason.
    store
        .append_event(
            &ctx,
            &journal_event(&ctx, "edge-never-enrolled", "REJECTED", Some("duplicate identity")),
        )
        .expect("the REJECTED event is recordable");
    let events = store
        .list_events(&ctx, "edge-never-enrolled", 100)
        .expect("reads");
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].event_type, "REJECTED");
    assert_eq!(events[0].reason.as_deref(), Some("duplicate identity"));
}
