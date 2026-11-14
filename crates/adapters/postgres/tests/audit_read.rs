// P4.36 — Integration tests for the audit-events READ port on real
//         PostgreSQL (report §15.2, §16.2, §21.3; ledger P4.36; decision
//         record: `ledger/phase-04-grid-feasibility-dispatch/36_audit_read_api.md`).
//
// Covered on real PostgreSQL (gated on FLEXCORE_TEST_DATABASE_URL, the
// recorded harness decision — every test skips cleanly when unset; UNRUN
// in this environment: nothing was built or executed here):
//
// - **Round trip through the read port (§15.2)**: audit records appended
//   through the P1.10 `AuditSink` (the ONLY write path — the read port has
//   no write method) read back through `AuditEventReader` with the §10.1
//   field set END TO END: the decision (outcome + reason reference, never
//   a bare allow/deny), the evidence reference, the before/after
//   references, the distinct P0.09 event/ingest times, the §21.3
//   correlation id, and the P1.10 correction linkage
//   (`corrected_audit_id`).
// - **Tenant isolation (§16.2)**: a foreign-tenant record reads as `None`
//   and leaks nothing into listings (RLS via the transaction-scoped
//   `flexcore.tenant_id` GUC + the explicit tenant filter, P0.16 rule 5).
// - **Listing discipline**: ordered by `(event_time, audit_id)`, exact
//   action/actor/object filters, INCLUSIVE time-window bounds (the
//   recorded P4.36 DEFAULT decision), and the keyset cursor
//   (`after_audit_id`) pages without gaps or overlaps.
//
// Roles: the runtime connects as the configured URL's role; the migration
// runner needs the owner role (`flexcore_migrator`) or a bootstrap
// administrator, per the P1.13 conventions.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

use flexcore_audit::record::{
    ActorIdentity, AuditAction, AuditDecision, AuditObject, AuditRecord, AuditRecordFields,
    DecisionOutcome, EvidenceReference, RecordReference,
};
use flexcore_audit::sink::AuditSink;
use flexcore_contracts::ids::FlexUuid;
use flexcore_contracts::time_model::{EventTime, IngestTime};
use flexcore_domain::entities::Tenant;
use flexcore_domain::ids::{AssetId, TenantId};
use flexcore_domain::tenant_context::{AuthenticatedClaims, TenantContext, VerifiedClaims};
use flexcore_ports::repository::TenantRepository;
use flexcore_postgres_adapter::FlexCoreStore;
use flexcore_storage::audit_read::{AuditEventFilter, AuditEventReader, AUDIT_DECISION_ALLOWED};

// --- Harness helpers (the recorded P2.06/P3.21/P4.10/P4.30 decisions) ---------

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
    bytes[1] = 0xC4; // P4.36 fixture marker
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
            subject: "p4.36-audit-read-test".to_string(),
        },
    ))
}

/// Seeds the tenant row the `audit_records.tenant_id` FK requires in one
/// committed unit of work (RLS scopes the insert to the caller's tenant).
fn seed_tenant(store: &FlexCoreStore, tenant_uuid: FlexUuid) -> TenantContext {
    let ctx = tenant_context(tenant_uuid);
    let created = EventTime::from_unix_millis(1_700_000_000_000).expect("valid");
    let unit: flexcore_postgres_adapter::PgUnitOfWork =
        store.begin(Some(&ctx)).expect("unit of work opens");
    let tenant = Tenant::try_new(
        TenantId::new(tenant_uuid),
        "p4.36 audit-read tenant".to_string(),
        None,
        format!("idp-{tenant_uuid}"),
        "default".to_string(),
        created,
    )
    .expect("tenant builds");
    TenantRepository::insert(&unit, &tenant).expect("tenant inserts");
    unit.commit().expect("committed");
    ctx
}

/// One appended audit record (through the P1.10 `AuditSink` — the ONLY
/// write path; the read port has no write method, P4.36).
fn appended_record(
    ctx: &TenantContext,
    audit_id: FlexUuid,
    actor: &str,
    action: &str,
    object_type: &str,
    object_id: &str,
    after_ref: Option<&str>,
    evidence: Option<&str>,
    event_millis: i64,
    ingest_millis: i64,
) -> AuditRecord {
    let fields = AuditRecordFields {
        audit_id,
        tenant_id: Some(ctx.tenant_id()),
        actor: ActorIdentity::new(actor.to_string()).expect("valid actor"),
        action: AuditAction::new(action.to_string()).expect("valid action"),
        object: AuditObject::new(object_type.to_string(), object_id.to_string())
            .expect("valid object"),
        before_ref: None,
        after_ref: after_ref.map(|r| RecordReference::new(r.to_string()).expect("valid ref")),
        decision: None,
        evidence: evidence.map(|e| EvidenceReference::new(e.to_string()).expect("valid evidence")),
        event_time: EventTime::from_unix_millis(event_millis).expect("valid event time"),
        ingest_time: IngestTime::from_unix_millis(ingest_millis).expect("valid ingest time"),
        correlation_id: fixture_uuid(0xF1),
    };
    AuditRecord::new(fields).expect("valid record")
}

// --- Tests ---------------------------------------------------------------------

#[test]
fn audit_records_round_trip_through_the_read_port_with_every_reference_preserved() {
    let Some(url) =
        require_db("audit_records_round_trip_through_the_read_port_with_every_reference_preserved")
    else {
        return;
    };
    let store = FlexCoreStore::connect(&url).expect("connects");
    store.run_migrations().expect("migrations apply");
    let tenant_uuid = fixture_uuid(0xB1);
    let ctx = seed_tenant(&store, tenant_uuid);

    let original_id = fixture_uuid(0xA1);
    let correction_id = fixture_uuid(0xA2);
    let plain_id = fixture_uuid(0xA3);
    let command = fixture_uuid(0xC1);

    // ONE unit of work appends three records through the P1.10 AuditSink:
    // an authorization decision record (outcome + reason reference —
    // never a bare allow/deny), a NEW correction record referencing it
    // (P1.10 step 3), and a plain registration record.
    {
        let unit = store.begin(Some(&ctx)).expect("unit of work opens");
        let mut decision_record = appended_record(
            &ctx,
            original_id,
            "user:operator-1",
            "command.approve",
            "command",
            &command.to_string(),
            Some(&format!("command:{command}:state=APPROVED")),
            Some("evidence:approval/apr-1"),
            1_700_000_100_000,
            1_700_000_100_500,
        );
        decision_record.decision = Some(
            AuditDecision::new(DecisionOutcome::Allowed, "approval-record:apr-1".to_string())
                .expect("valid decision"),
        );
        unit.append(decision_record)
            .expect("the decision record appends");
        let correction = appended_record(
            &ctx,
            correction_id,
            "svc:audit-worker",
            "audit.correct",
            "audit_record",
            &original_id.to_string(),
            None,
            None,
            1_700_000_200_000,
            1_700_000_200_500,
        )
        .link_correction(original_id, "wrong after-ref recorded".to_string())
        .expect("the correction links");
        unit.append(correction).expect("the correction appends");
        unit.append(appended_record(
            &ctx,
            plain_id,
            "svc:registry",
            "asset.register",
            "asset",
            &AssetId::new(fixture_uuid(0xD1)).uuid().to_string(),
            None,
            None,
            1_700_000_050_000,
            1_700_000_050_100,
        ))
        .expect("the plain record appends");
        unit.commit().expect("committed");
    }

    // A SECOND unit of work reads back through the SELECT-only read port:
    // every §10.1 field end to end, references preserved (never a summary
    // that drops them — P5.09 consumes them).
    {
        let unit = store.begin(Some(&ctx)).expect("unit of work opens");
        let record = AuditEventReader::find_audit_event(
            &unit,
            &ctx,
            &original_id.to_string(),
        )
        .expect("read succeeds")
        .expect("the decision record is found");
        assert_eq!(record.audit_id, original_id.to_string());
        assert_eq!(record.tenant_id.map(|t| t.uuid()), Some(tenant_uuid));
        assert_eq!(record.actor, "user:operator-1");
        assert_eq!(record.action, "command.approve");
        assert_eq!(record.object_type, "command");
        assert_eq!(record.object_id, command.to_string());
        assert_eq!(
            record.after_ref.as_deref(),
            Some(format!("command:{command}:state=APPROVED").as_str()),
            "the after reference is preserved as a pointer (P1.12 s4.7)"
        );
        assert_eq!(record.decision_outcome.as_deref(), Some(AUDIT_DECISION_ALLOWED));
        assert_eq!(
            record.decision_reason_reference.as_deref(),
            Some("approval-record:apr-1"),
            "a decision is outcome + reason reference, never a bare allow (P1.11)"
        );
        assert_eq!(record.evidence.as_deref(), Some("evidence:approval/apr-1"));
        assert_eq!(record.event_time_unix_millis, 1_700_000_100_000);
        assert_eq!(record.ingest_time_unix_millis, 1_700_000_100_500);
        assert!(record.corrected_audit_id.is_none());

        // The correction linkage survives end to end: a NEW record
        // referencing the original (P1.10 step 3).
        let correction = AuditEventReader::find_audit_event(
            &unit,
            &ctx,
            &correction_id.to_string(),
        )
        .expect("read succeeds")
        .expect("the correction record is found");
        assert_eq!(
            correction.corrected_audit_id.as_deref(),
            Some(original_id.to_string()).as_deref(),
            "the correction references the original — the original is never edited"
        );
        // The original record is UNCHANGED on its read-back (append-only).
        let original_again = AuditEventReader::find_audit_event(
            &unit,
            &ctx,
            &original_id.to_string(),
        )
        .expect("read succeeds")
        .expect("the original is still there");
        assert_eq!(original_again.corrected_audit_id, None);

        // The §21.3 correlation id travels on every record.
        assert!(!record.correlation_id.is_empty());
        unit.commit().expect("committed");
    }
}

#[test]
fn a_foreign_tenant_audit_record_reads_as_none_and_leaks_nothing() {
    let Some(url) = require_db("a_foreign_tenant_audit_record_reads_as_none_and_leaks_nothing")
    else {
        return;
    };
    let store = FlexCoreStore::connect(&url).expect("connects");
    store.run_migrations().expect("migrations apply");
    let tenant_a = fixture_uuid(0xB2);
    let tenant_b = fixture_uuid(0xB3);
    let ctx_a = seed_tenant(&store, tenant_a);
    let ctx_b = seed_tenant(&store, tenant_b);

    let record_id = fixture_uuid(0xA4);
    {
        let unit = store.begin(Some(&ctx_b)).expect("unit of work opens");
        unit.append(appended_record(
            &ctx_b,
            record_id,
            "user:admin-b",
            "asset.register",
            "asset",
            &AssetId::new(fixture_uuid(0xD2)).uuid().to_string(),
            None,
            None,
            1_700_000_100_000,
            1_700_000_100_100,
        ))
        .expect("tenant B's record appends");
        unit.commit().expect("committed");
    }

    // Tenant A: the foreign-tenant record reads as None and is absent
    // from the listing — existence never leaks (§16.2, P0.16 rule 5; RLS
    // plus the explicit tenant filter).
    {
        let unit = store.begin(Some(&ctx_a)).expect("unit of work opens");
        let found =
            AuditEventReader::find_audit_event(&unit, &ctx_a, &record_id.to_string())
                .expect("read succeeds");
        assert!(found.is_none(), "a foreign-tenant record reads as None");
        let listed = AuditEventReader::list_audit_events(
            &unit,
            &ctx_a,
            &AuditEventFilter::default(),
            100,
        )
        .expect("listing succeeds");
        assert!(
            listed.is_empty(),
            "a foreign-tenant record leaks nothing into listings"
        );
        unit.commit().expect("committed");
    }
    // Tenant B still reads its own record back.
    {
        let unit = store.begin(Some(&ctx_b)).expect("unit of work opens");
        let found =
            AuditEventReader::find_audit_event(&unit, &ctx_b, &record_id.to_string())
                .expect("read succeeds");
        assert!(found.is_some(), "the owning tenant reads its record");
        let listed = AuditEventReader::list_audit_events(
            &unit,
            &ctx_b,
            &AuditEventFilter::default(),
            100,
        )
        .expect("listing succeeds");
        assert_eq!(listed.len(), 1, "only the caller's tenant's records");
        unit.commit().expect("committed");
    }
}

#[test]
fn the_listing_filters_time_window_and_keyset_cursor_scope_the_audit_read() {
    let Some(url) =
        require_db("the_listing_filters_time_window_and_keyset_cursor_scope_the_audit_read")
    else {
        return;
    };
    let store = FlexCoreStore::connect(&url).expect("connects");
    store.run_migrations().expect("migrations apply");
    let tenant_uuid = fixture_uuid(0xB4);
    let ctx = seed_tenant(&store, tenant_uuid);
    let command = fixture_uuid(0xC2);

    let first = fixture_uuid(0xA5);
    let second = fixture_uuid(0xA6);
    let third = fixture_uuid(0xA7);
    {
        let unit = store.begin(Some(&ctx)).expect("unit of work opens");
        // Three approval records on the same command, distinct event times.
        for (id, millis) in [
            (first, 1_700_000_100_000),
            (second, 1_700_000_200_000),
            (third, 1_700_000_300_000),
        ] {
            unit.append(appended_record(
                &ctx,
                id,
                "user:operator-1",
                "command.approve",
                "command",
                &command.to_string(),
                None,
                None,
                millis,
                millis + 500,
            ))
            .expect("the record appends");
        }
        // One record OUTSIDE the action filter's class.
        unit.append(appended_record(
            &ctx,
            fixture_uuid(0xA8),
            "user:operator-1",
            "asset.register",
            "asset",
            &AssetId::new(fixture_uuid(0xD3)).uuid().to_string(),
            None,
            None,
            1_700_000_250_000,
            1_700_000_250_500,
        ))
        .expect("the off-class record appends");
        unit.commit().expect("committed");
    }

    {
        let unit = store.begin(Some(&ctx)).expect("unit of work opens");

        // The action (event class) filter scopes to command.approve only,
        // in (event_time, audit_id) order.
        let listed = AuditEventReader::list_audit_events(
            &unit,
            &ctx,
            &AuditEventFilter {
                action: Some("command.approve".to_string()),
                ..AuditEventFilter::default()
            },
            100,
        )
        .expect("listing succeeds");
        let ids: Vec<&str> = listed.iter().map(|r| r.audit_id.as_str()).collect();
        assert_eq!(
            ids,
            vec![
                first.to_string(),
                second.to_string(),
                third.to_string(),
            ],
            "only the filtered class, in event-time order"
        );

        // The INCLUSIVE time window (the recorded P4.36 DEFAULT decision):
        // both bounds hit.
        let windowed = AuditEventReader::list_audit_events(
            &unit,
            &ctx,
            &AuditEventFilter {
                from_event_unix_millis: Some(1_700_000_100_000),
                to_event_unix_millis: Some(1_700_000_200_000),
                ..AuditEventFilter::default()
            },
            100,
        )
        .expect("listing succeeds");
        assert_eq!(windowed.len(), 2, "the inclusive bounds are honoured");

        // The keyset cursor: page one (limit 2), then page two strictly
        // after the last record of page one — no gaps, no overlaps.
        let page_one = AuditEventReader::list_audit_events(
            &unit,
            &ctx,
            &AuditEventFilter::default(),
            2,
        )
        .expect("page one succeeds");
        assert_eq!(page_one.len(), 2);
        let cursor = page_one.last().expect("page one has a last record").audit_id.clone();
        let page_two = AuditEventReader::list_audit_events(
            &unit,
            &ctx,
            &AuditEventFilter {
                after_audit_id: Some(cursor),
                ..AuditEventFilter::default()
            },
            100,
        )
        .expect("page two succeeds");
        let page_two_ids: Vec<&str> = page_two.iter().map(|r| r.audit_id.as_str()).collect();
        assert!(
            !page_two_ids.contains(&page_one[0].audit_id.as_str())
                && !page_two_ids.contains(&page_one[1].audit_id.as_str()),
            "page two never repeats page one"
        );
        assert_eq!(page_two.len(), 2, "the remaining records page through");

        // The object filters scope to one entity (the command/schedule
        // filter shape of ledger P4.36 step 2).
        let by_object = AuditEventReader::list_audit_events(
            &unit,
            &ctx,
            &AuditEventFilter {
                object_type: Some("command".to_string()),
                object_id: Some(command.to_string()),
                ..AuditEventFilter::default()
            },
            100,
        )
        .expect("listing succeeds");
        assert_eq!(by_object.len(), 3, "the object pair scopes the listing");
        unit.commit().expect("committed");
    }
}
