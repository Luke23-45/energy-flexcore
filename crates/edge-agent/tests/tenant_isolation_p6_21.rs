// P6.21 — Multi-site and multi-tenant isolation at the edge: EDGE-SIDE
//         tests (report §16.2, §17.4, §17.5 rule 8, §5.7; ledger P6.21).
//         Written, NOT run — the loop bans in
//         `ledger/phase-06-edge-protocol/ENTRY_NOTE.md` hold. The human
//         build/test pass runs
//         `cargo test -p flexcore-edge-agent --test tenant_isolation_p6_21`.
//
// The isolation subject: edge A holds ONE bound identity (the P6.06
// `DeviceBinding` — one tenant, one approved site, §16.2). Tenant/site
// scoping comes from THAT bound identity — never from any request-carried
// field (the §16.2 warning is enforced here by construction: the guard
// below takes no request-scope input at all).
//
// What is proven (the ledger step-1 negative set — every denial audited
// with actor, object, and timestamp through the P6.18 durable audit
// buffer):
//
// 1. Edge A requests tenant B's telemetry → DENIED, audited
//    (`EDGE_SCOPE_DENIED`).
// 2. Edge A presents a command for site B → DENIED twice over, read-only
//    through the P6.14 gate surface (no edits to `command_path.rs` — the
//    denial path is constructed here from the public API):
//    (a) the DEFAULT gate refuses every command outright
//        (`command_path_disabled`), and
//    (b) even a gate ENABLED for site A (test-env, all four conditions)
//        refuses a command whose target adapter belongs to site B
//        (`adapter_not_allowlisted` — the per-site allowlist is the
//        per-command check), each with an audited `EDGE_COMMAND_REJECTED`
//        P4-pattern denial record, plus an explicit `EDGE_SCOPE_DENIED`
//        record naming actor/object.
// 3. Edge A enumerates assets outside its scope → DENIED, audited.
// 4. Scoping from the BOUND IDENTITY: a request-carried scope claim is
//    ignored (the guard has no parameter for it); an incomplete binding
//    fails closed for EVERYTHING.
//
// The server-side half of P6.21 (a malicious client driving the SERVER
// endpoints directly, never relying on edge goodwill) is
// `apps/control-plane/tests/api_edge_isolation_p6_21.rs`; the blast-radius
// note and the human-gate inputs (modified-edge-client lab run, RLS
// app-role ownership review) are recorded in the decision document
// `ledger/phase-06-edge-protocol/18_21_audit_offline_isolation.md`.

use std::sync::Arc;

use flexcore_edge_agent::audit_buffer::{BufferedAuditKind, BufferedAuditRecord, FileAuditBuffer};
use flexcore_edge_agent::command_path::{
    CommandPathGateInputs, CommandPathRuntime, CommandReceiver, EdgeCommand, EnablementKey,
    SignedEnablementRecord,
};
use flexcore_edge_agent::enrollment::DeviceBinding;
use flexcore_edge_agent::policy::{
    AdapterPolicy, OperatingWindow, RateLimit, SitePolicyConfig, SitePolicyEngine,
};
use flexcore_edge_agent::ports::AuditSink;
use flexcore_ports::clock::Clock;
use flexcore_ports::fakes::FixedClock;

const BASE_MILLIS: i64 = 1_700_000_000_000;

const EDGE_A_TENANT: &str = "11111111-1111-4111-8111-111111111111";
const EDGE_A_SITE: &str = "aaaaaaaa-0000-4000-8000-00000000a001";
const TENANT_B: &str = "22222222-2222-4222-8222-222222222222";
const SITE_B: &str = "bbbbbbbb-0000-4000-8000-00000000b002";
const EDGE_A_ACTOR: &str = "edge:pilot-site-A:edge-01";

/// A std-only unique scratch directory per call.
fn scratch_dir(label: &str) -> std::path::PathBuf {
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let dir = std::env::temp_dir().join(format!(
        "flexcore-edge-p621-{}-{}-{}",
        std::process::id(),
        label,
        COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst)
    ));
    std::fs::create_dir_all(&dir).expect("scratch dir creates");
    dir
}

fn clock_at(millis: i64) -> Arc<dyn Clock + Send + Sync> {
    Arc::new(FixedClock::at_unix_millis(millis))
}

// ---------------------------------------------------------------------------
// The edge-side scope guard (the P6.21 denial path, built ONLY from public
// API: the P6.06 bound identity + the P6.18 durable audit buffer).
// ---------------------------------------------------------------------------

/// A named denial. The denial itself is ALWAYS audited before it is
/// returned (audit-or-refuse, never silently denied).
#[derive(Debug, Clone, PartialEq, Eq)]
struct ScopeDenial {
    code: &'static str,
    actor: String,
    object: String,
}

/// The edge-side scope guard. The scope source is EXCLUSIVELY the edge's
/// bound identity ([`DeviceBinding`] — the P6.06 credential/identity
/// context). There is deliberately NO parameter and NO field through which
/// a request could carry, override, or widen the scope (§16.2: tenant
/// context is never taken from an untrusted request field).
struct EdgeScopeGuard {
    binding: DeviceBinding,
    /// The durable audit buffer — every denial is recorded with actor,
    /// object, and the stamped timestamp, and SURVIVES a central outage
    /// (P6.18: the denial evidence uploads on reconnect, in order).
    audit: Arc<FileAuditBuffer>,
}

impl EdgeScopeGuard {
    /// Composes the guard over the bound identity. An INCOMPLETE binding
    /// composes fine but denies everything (fail-closed — see `check`).
    fn new(binding: DeviceBinding, audit: Arc<FileAuditBuffer>) -> Self {
        Self { binding, audit }
    }

    /// The single fail-closed check behind every surface (telemetry read,
    /// asset enumeration, command presentation). `code` names the denied
    /// surface; `actor` is the principal (the edge identity the request
    /// was authenticated as); `object` is the requested tenant/site/asset.
    fn check(
        &self,
        code: &'static str,
        actor: &str,
        object: &str,
        requested_tenant: &str,
        requested_site: &str,
    ) -> Result<(), ScopeDenial> {
        // Fail-closed: an incomplete bound identity denies EVERYTHING (an
        // edge that cannot name its scope has nothing approved).
        let allowed = self.binding.is_complete()
            && requested_tenant == self.binding.tenant_id
            && requested_site == self.binding.site_id;
        if allowed {
            return Ok(());
        }
        let denial = ScopeDenial {
            code,
            actor: actor.to_string(),
            object: object.to_string(),
        };
        // AUDIT BEFORE REFUSE (§17.3): the dedicated `EDGE_SCOPE_DENIED`
        // name leads the detail; the record carries actor, object, and the
        // stamped timestamp in the durable buffer. No secret ever enters
        // these fields (ids only).
        let detail = format!(
            "EDGE_SCOPE_DENIED: code={} actor={} object={} requested_tenant={} requested_site={} \
             bound_tenant={} bound_site={} (scoping from the bound identity only — §16.2)",
            denial.code, denial.actor, denial.object, requested_tenant, requested_site,
            self.binding.tenant_id, self.binding.site_id,
        );
        self.audit
            .record_event(BufferedAuditKind::ScopeDenied, actor, object, detail)
            .expect("the denial audit write is part of the denial path");
        Err(denial)
    }

    /// Edge A requests tenant/site telemetry.
    fn request_telemetry(
        &self,
        actor: &str,
        requested_tenant: &str,
        requested_site: &str,
        object: &str,
    ) -> Result<(), ScopeDenial> {
        self.check("cross_tenant_telemetry", actor, object, requested_tenant, requested_site)
    }

    /// Edge A enumerates assets of a tenant/site.
    fn enumerate_assets(
        &self,
        actor: &str,
        requested_tenant: &str,
        requested_site: &str,
        object: &str,
    ) -> Result<(), ScopeDenial> {
        self.check("cross_site_asset_enumeration", actor, object, requested_tenant, requested_site)
    }
}

// --- P6.14 gate surface helpers (read-only use of the public API) ------------

fn key() -> EnablementKey {
    EnablementKey::new(
        "lab-enablement-key-material".to_string(),
        "deployment:lab-key-1".to_string(),
    )
    .expect("valid key")
}

/// A signed, time-bounded enablement record bound to EDGE A's SITE — the
/// all-four-conditions test-env gate, composed per the P6.14 procedure.
fn site_a_enablement_inputs() -> CommandPathGateInputs {
    let key = key();
    let record = SignedEnablementRecord {
        site_id: EDGE_A_SITE.to_string(),
        approver: "J. Doe (security lead)".to_string(),
        adapter_scope: vec!["simulated-test".to_string()],
        enablement_token: "lab-token-123".to_string(),
        issued_at_unix_millis: BASE_MILLIS,
        valid_until_unix_millis: BASE_MILLIS + 3_600_000,
        key_reference: "deployment:lab-key-1".to_string(),
        digest: None,
    }
    .sign(&key)
    .expect("signs");
    CommandPathGateInputs {
        // The binding comes from the edge's configured identity (the same
        // bound site the guard reads) — never from any command/request.
        site_id: EDGE_A_SITE.to_string(),
        config_flag_enabled: true,
        signed_record: Some(record),
        environment_token: Some("lab-token-123".to_string()),
        // The SITE A command allowlist: ONLY site A's adapter ids. Site B's
        // adapters are not on it — and never can be from a request.
        command_allowlist: vec!["simulated-test".to_string()],
        d6_record_reference: Some(
            "ledger/phase-06-edge-protocol/03_d6_boundary_review.md".to_string(),
        ),
    }
}

fn permissive_policy_for_site_a() -> Arc<SitePolicyEngine> {
    Arc::new(
        SitePolicyEngine::new(SitePolicyConfig {
            format_version: 1,
            adapters: vec![AdapterPolicy {
                adapter_id: "simulated-test".to_string(),
                enabled: true,
                rate_limit: RateLimit {
                    max_commands: 100,
                    window_millis: 60_000,
                },
                operating_window: OperatingWindow {
                    start_millis_of_day: 0,
                    end_millis_of_day: 86_400_000,
                },
            }],
        })
        .expect("valid policy"),
    )
}

fn issued_command(id: &str, asset_key: &str) -> EdgeCommand {
    EdgeCommand::new(id.to_string(), asset_key.to_string(), "ISSUED".to_string(), 1)
        .expect("valid command")
}

/// All ScopeDenied records currently pending in the buffer.
fn scope_denials(buffer: &FileAuditBuffer) -> Vec<BufferedAuditRecord> {
    let pending = buffer.drain(1_000);
    let denials: Vec<BufferedAuditRecord> = pending
        .iter()
        .filter(|r| r.kind == BufferedAuditKind::ScopeDenied)
        .cloned()
        .collect();
    buffer.prepend(pending);
    denials
}

// --- 1. Edge A requests tenant B's telemetry ----------------------------------

#[test]
fn edge_a_requesting_tenant_b_telemetry_is_denied_and_audited() {
    let dir = scratch_dir("tenant-b-telemetry");
    let journal = dir.join("audit.journal");
    let buffer = Arc::new(FileAuditBuffer::open(&journal, 1_000, clock_at(BASE_MILLIS)).expect("opens"));
    let guard = EdgeScopeGuard::new(
        DeviceBinding::new(EDGE_A_TENANT.to_string(), EDGE_A_SITE.to_string()),
        Arc::clone(&buffer),
    );

    // The denial.
    let error = guard
        .request_telemetry(
            EDGE_A_ACTOR,
            TENANT_B,
            SITE_B,
            "telemetry:tenant-B:site-B:ess-01:active_power",
        )
        .expect_err("cross-tenant telemetry is denied");
    assert_eq!(error.code, "cross_tenant_telemetry");

    // Within the bound scope the same surface PASSES (the guard is a
    // scope check, not a blanket denial).
    guard
        .request_telemetry(
            EDGE_A_ACTOR,
            EDGE_A_TENANT,
            EDGE_A_SITE,
            "telemetry:tenant-A:site-A:ess-01:active_power",
        )
        .expect("the bound scope is allowed");

    // The denial is audited with actor, object, and timestamp — and the
    // ALLOWED access is not.
    let denials = scope_denials(&buffer);
    assert_eq!(denials.len(), 1, "exactly one denial was audited");
    assert_eq!(denials[0].kind.name(), "EDGE_SCOPE_DENIED");
    assert_eq!(denials[0].actor, EDGE_A_ACTOR);
    assert_eq!(denials[0].object, "telemetry:tenant-B:site-B:ess-01:active_power");
    assert_eq!(denials[0].recorded_at.unix_millis(), BASE_MILLIS as i128);
    assert!(denials[0].detail.contains("code=cross_tenant_telemetry"));
    assert!(denials[0].detail.contains(TENANT_B));
    cleanup(&dir);
}

// --- 2. Edge A presents a command for site B -----------------------------------

#[test]
fn edge_a_presenting_a_command_for_site_b_is_refused_by_the_p6_14_gate_surface() {
    let dir = scratch_dir("site-b-command");
    let journal = dir.join("audit.journal");
    let buffer = Arc::new(FileAuditBuffer::open(&journal, 1_000, clock_at(BASE_MILLIS)).expect("opens"));
    let guard = EdgeScopeGuard::new(
        DeviceBinding::new(EDGE_A_TENANT.to_string(), EDGE_A_SITE.to_string()),
        Arc::clone(&buffer),
    );
    // The command targets a SITE B asset (site B's asset key) via SITE B's
    // adapter id — the cross-site presentation.
    let site_b_command = issued_command("cmd-x-site", "site-b-asset-01");
    let site_b_adapter = "site-b-ems-adapter";

    // (a) The DEFAULT (disabled) gate refuses outright — the default build
    //     denies every command, cross-site or not, and audits the refusal.
    let default_runtime =
        CommandPathRuntime::evaluate(&CommandPathGateInputs::default(), &key(), BASE_MILLIS);
    let mut default_receiver = CommandReceiver::new(
        default_runtime,
        permissive_policy_for_site_a(),
        Arc::clone(&buffer) as Arc<dyn AuditSink>,
        clock_at(BASE_MILLIS + 1_000),
    );
    let refusal = default_receiver
        .receive(&site_b_command, site_b_adapter)
        .expect_err("the default gate refuses");
    assert_eq!(refusal.code(), "command_path_disabled");

    // (b) Even with the gate ENABLED for site A (all four P6.14 conditions,
    //     test-env, signed, time-bounded), the SITE B command is refused by
    //     the per-command allowlist check — the enablement's scope is site
    //     A's allowlist and site B's adapter is not on it.
    let enabled_runtime =
        CommandPathRuntime::evaluate(&site_a_enablement_inputs(), &key(), BASE_MILLIS);
    assert!(enabled_runtime
        .enablement_at(BASE_MILLIS + 1_000)
        .is_enabled_at(BASE_MILLIS + 1_000));
    let mut enabled_receiver = CommandReceiver::new(
        enabled_runtime,
        permissive_policy_for_site_a(),
        Arc::clone(&buffer) as Arc<dyn AuditSink>,
        clock_at(BASE_MILLIS + 2_000),
    );
    let refusal = enabled_receiver
        .receive(&site_b_command, site_b_adapter)
        .expect_err("a site-B command is outside site A's allowlist");
    assert_eq!(refusal.code(), "adapter_not_allowlisted");

    // The explicit scope-denial audit for the cross-site presentation
    // (actor, object, timestamp in the durable buffer).
    let error = guard
        .check(
            "cross_site_command",
            EDGE_A_ACTOR,
            "command:cmd-x-site:site-b-asset-01",
            TENANT_B,
            SITE_B,
        )
        .expect_err("the cross-site command is denied at the scope layer too");
    assert_eq!(error.code, "cross_site_command");

    // Audit evidence: two EDGE_COMMAND_REJECTED records (one per refusal,
    // each with the P4-pattern denial record) + one EDGE_SCOPE_DENIED.
    let pending = buffer.drain(1_000);
    let rejections: Vec<&BufferedAuditRecord> = pending
        .iter()
        .filter(|r| r.kind == BufferedAuditKind::CommandRejected)
        .collect();
    assert_eq!(rejections.len(), 2, "both refusals are audited");
    assert!(rejections[0].detail.contains("command_denial"));
    assert!(rejections[0].detail.contains("code=command_path_disabled"));
    assert!(rejections[1].detail.contains("code=adapter_not_allowlisted"));
    assert!(rejections[1].detail.contains("at="));
    let denials: Vec<&BufferedAuditRecord> = pending
        .iter()
        .filter(|r| r.kind == BufferedAuditKind::ScopeDenied)
        .collect();
    assert_eq!(denials.len(), 1);
    assert_eq!(denials[0].actor, EDGE_A_ACTOR);
    assert_eq!(denials[0].object, "command:cmd-x-site:site-b-asset-01");
    assert_eq!(denials[0].recorded_at.unix_millis(), BASE_MILLIS as i128);
    // NOTHING was admitted: no EDGE_COMMAND_ACCEPTED record exists.
    assert!(
        !pending
            .iter()
            .any(|r| r.kind == BufferedAuditKind::CommandAccepted),
        "a cross-site command is never admitted"
    );
    cleanup(&dir);
}

// --- 3. Edge A enumerates assets outside its scope ------------------------------

#[test]
fn edge_a_enumerating_assets_outside_its_scope_is_denied_and_audited() {
    let dir = scratch_dir("asset-enumeration");
    let journal = dir.join("audit.journal");
    let buffer = Arc::new(FileAuditBuffer::open(&journal, 1_000, clock_at(BASE_MILLIS)).expect("opens"));
    let guard = EdgeScopeGuard::new(
        DeviceBinding::new(EDGE_A_TENANT.to_string(), EDGE_A_SITE.to_string()),
        Arc::clone(&buffer),
    );

    // Tenant B (wrong tenant, wrong site).
    let error = guard
        .enumerate_assets(EDGE_A_ACTOR, TENANT_B, SITE_B, "assets:tenant-B")
        .expect_err("cross-tenant enumeration is denied");
    assert_eq!(error.code, "cross_site_asset_enumeration");
    // Same tenant, WRONG SITE (a cross-site escalation attempt).
    let error = guard
        .enumerate_assets(EDGE_A_ACTOR, EDGE_A_TENANT, SITE_B, "assets:tenant-A:site-B")
        .expect_err("cross-site enumeration inside the tenant is denied too");
    assert_eq!(error.code, "cross_site_asset_enumeration");
    // Within scope: allowed.
    guard
        .enumerate_assets(EDGE_A_ACTOR, EDGE_A_TENANT, EDGE_A_SITE, "assets:tenant-A:site-A")
        .expect("the bound site's assets are enumerable");

    let denials = scope_denials(&buffer);
    assert_eq!(denials.len(), 2, "both denials are audited");
    for denial in &denials {
        assert_eq!(denial.actor, EDGE_A_ACTOR);
        assert!(!denial.object.is_empty());
        assert_eq!(denial.recorded_at.unix_millis(), BASE_MILLIS as i128);
    }
    cleanup(&dir);
}

// --- 4. Scoping from the bound identity, never a request-carried field ---------

#[test]
fn scoping_comes_from_the_bound_identity_never_a_request_carried_field() {
    let dir = scratch_dir("bound-identity");
    let journal = dir.join("audit.journal");
    let buffer = Arc::new(FileAuditBuffer::open(&journal, 1_000, clock_at(BASE_MILLIS)).expect("opens"));

    // A malicious request CLAIMS tenant A in its own scope field. The
    // guard has no parameter for that claim — the scope is the BOUND
    // identity, so a tenant-B object stays denied no matter what the
    // request says about itself.
    let guard = EdgeScopeGuard::new(
        DeviceBinding::new(EDGE_A_TENANT.to_string(), EDGE_A_SITE.to_string()),
        Arc::clone(&buffer),
    );
    guard
        .request_telemetry(EDGE_A_ACTOR, TENANT_B, SITE_B, "claims_tenant_A_in_its_body")
        .expect_err("a request-carried scope claim cannot widen the bound scope");

    // An edge whose binding is INCOMPLETE denies EVERYTHING (fail-closed),
    // including objects that "look" like its own scope.
    let broken = EdgeScopeGuard::new(
        DeviceBinding::new("  ".to_string(), EDGE_A_SITE.to_string()),
        Arc::clone(&buffer),
    );
    broken
        .request_telemetry(EDGE_A_ACTOR, EDGE_A_TENANT, EDGE_A_SITE, "incomplete-binding")
        .expect_err("an incomplete binding fails closed");
    broken
        .enumerate_assets(EDGE_A_ACTOR, EDGE_A_TENANT, EDGE_A_SITE, "incomplete-binding-assets")
        .expect_err("an incomplete binding fails closed for every surface");

    let denials = scope_denials(&buffer);
    assert_eq!(denials.len(), 3, "every denial is audited");
    assert!(denials.iter().all(|d| d.kind.name() == "EDGE_SCOPE_DENIED"));
    cleanup(&dir);
}

// --- 5. Denial evidence survives a central outage (P6.18 composition) ----------

#[test]
fn isolation_denials_are_durably_audited_and_upload_in_order_after_reconnect() {
    let dir = scratch_dir("denials-durable");
    let journal = dir.join("audit.journal");

    // Phase 1 — the central channel is DOWN: the denials still happen and
    // are still audited (the audit buffer is local and durable).
    {
        let buffer = Arc::new(FileAuditBuffer::open(&journal, 1_000, clock_at(BASE_MILLIS)).expect("opens"));
        let guard = EdgeScopeGuard::new(
            DeviceBinding::new(EDGE_A_TENANT.to_string(), EDGE_A_SITE.to_string()),
            Arc::clone(&buffer),
        );
        for n in 1..=3u64 {
            let _ = guard.request_telemetry(
                EDGE_A_ACTOR,
                TENANT_B,
                SITE_B,
                &format!("telemetry:tenant-B:object-{n}"),
            );
        }
        assert_eq!(scope_denials(&buffer).len(), 3);
        // Dropped WITHOUT commit: everything stays on disk.
    }

    // Phase 2 — reconnect: the denial evidence uploads IN ORDER, with
    // actor/object/timestamp intact (§17.3: the audit trail survives the
    // exact window an incident investigation needs).
    let buffer = FileAuditBuffer::open(&journal, 1_000, clock_at(BASE_MILLIS + 1_800_000)).expect("reopens");
    let pending = buffer.drain(1_000);
    let denials: Vec<&BufferedAuditRecord> = pending
        .iter()
        .filter(|r| r.kind == BufferedAuditKind::ScopeDenied)
        .collect();
    assert_eq!(denials.len(), 3, "all denial evidence survives the restart");
    assert_eq!(
        denials.iter().map(|r| r.sequence).collect::<Vec<_>>(),
        vec![1, 2, 3],
        "in-order upload after reconnect"
    );
    for (index, record) in denials.iter().enumerate() {
        assert_eq!(record.actor, EDGE_A_ACTOR);
        assert!(record.object.contains(&format!("object-{}", index + 1)));
        assert_eq!(record.recorded_at.unix_millis(), BASE_MILLIS as i128);
    }
    let committed = buffer.commit_uploaded(&pending).expect("commits");
    assert_eq!(committed, pending.len());
    cleanup(&dir);
}
