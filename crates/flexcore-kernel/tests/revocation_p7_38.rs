// P7.38 — Plugin revocation, staged rollout, and failed-plugin removal
// (test file WRITTEN, NOT RUN — the loop bans hold; the human build/test
// pass executes `cargo test -p flexcore-kernel --test revocation_p7_38`).
//
// Coverage (ledger P7.38 acceptance criteria; task test list):
//
// 1. Revoke-by-id blocks verify + start (the `Discovered → Verified`
//    gate consults the register FIRST; the registry-time re-check
//    refuses `Configured → Started`; the revoked plugin cannot
//    re-verify after an explicit restart either).
// 2. Revoke-by-key blocks ALL its plugins (the signer key reference is
//    the join key; a plugin with a different key still verifies).
// 3. Append-only register: no un-revoke (duplicate refused; the seam
//    statement pinned; no removal API exists on the store trait).
// 4. Staged promotion path + out-of-order promotion refusal (and the
//    no-flag-flip rule: promoting the last profile first is refused).
// 5. Explicit removal path with audit (Failed → Draining → Stopped,
//    reason + actor required, every step audited).
// 6. Failed-plugin removal preserves audit records (the lifecycle's
//    audit trail is untouched; no residual scheduled work remains).
// 7. Refusals: removal from a non-Failed state; removal drain deadline
//    exceeded (fail-closed restoration + containment instruction).
// 8. File-backed register: reopen survival, foreign-file refusal,
//    corrupt-tail truncate-and-count.
// 9. Gate order: revocation BEFORE provenance (a revoked plugin is
//    refused even when its provenance digest would verify).

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use flexcore_kernel::lifecycle::{
    DrainOutcome, DrainPolicy, DrainPort, FixedClock, GateOutcome, LifecycleState,
    MemoryAuditSink, PluginLifecycle,
};
use flexcore_kernel::manifest::SemVer;
use flexcore_kernel::revocation::{
    ensure_startable, is_revoked, remove_failed_plugin, revoke_plugin, revoke_signer_key,
    revocation_gate, FileRevocationRegister, MemoryRevocationAuditSink,
    MemoryRevocationRegister, RemovalError, RevocationAuditKind, RevocationError,
    RevocationKey, RevocationRecord, RevocationStore, RolloutError, StagedRollout,
    REMOVAL_STATEMENT, REVOCATION_STATEMENT, ROLLOUT_STATEMENT, UNREVOCATION_SEAM,
};
use flexcore_kernel::signing::{
    provenance_gate, ProvenanceRecord, UnsafeTestOnlyDigest, PROVENANCE_SCHEMA_ID,
};

const BASE_MS: u64 = 1_700_000_000_000;
const PLUGIN_ID: &str = "neryva.revoked-scheduler";
const KEY_REF: &str = "key-store:release-2026-11";
const OTHER_KEY_REF: &str = "key-store:release-2026-12";

fn version(major: u64, minor: u64, patch: u64) -> SemVer {
    SemVer::new(major, minor, patch)
}

/// A unique scratch path per call (std-only; the P7.36 test pattern).
fn scratch_path(label: &str) -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let mut path = std::env::temp_dir();
    path.push(format!(
        "flexcore-kernel-p738-{}-{}-{}",
        std::process::id(),
        label,
        COUNTER.fetch_add(1, Ordering::SeqCst)
    ));
    let _ = std::fs::remove_file(&path);
    path
}

/// A drain port whose outstanding work is cancellable (quiesces at the
/// deadline, or never quiesces when `stuck`).
struct TestDrain {
    outstanding: usize,
    stuck: bool,
}

impl TestDrain {
    fn with_work(outstanding: usize) -> TestDrain {
        TestDrain { outstanding, stuck: false }
    }
    fn stuck(outstanding: usize) -> TestDrain {
        TestDrain { outstanding, stuck: true }
    }
}

impl DrainPort for TestDrain {
    fn outstanding_work(&self) -> usize {
        self.outstanding
    }
    fn drain(&mut self, _deadline_millis: u64) -> DrainOutcome {
        if self.stuck {
            DrainOutcome::DeadlineExceeded { outstanding: self.outstanding }
        } else {
            let cancelled = self.outstanding;
            self.outstanding = 0;
            DrainOutcome::Quiesced { cancelled }
        }
    }
}

/// A provenance record bound to (plugin_id, version) with `key_ref` —
/// valid for the test digest over `artifact`.
fn provenance(plugin_id: &str, v: SemVer, key_ref: &str, artifact: &[u8]) -> ProvenanceRecord {
    let digest = {
        // Same FNV-1a 64 the test-only verifier computes (the signing
        // tests compute it independently; here we reuse the verifier by
        // constructing a record whose digest we ask the verifier for —
        // a tiny local copy keeps the fixture self-contained).
        let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
        for byte in artifact {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
        format!("{hash:016x}")
    };
    ProvenanceRecord {
        artifact_id: format!("artifacts/{plugin_id}-{v}.wasm"),
        digest_algorithm: "fnv1a64-test".to_string(),
        digest_hex: digest,
        signer_key_reference: key_ref.to_string(),
        signed_at_millis: BASE_MS,
        manifest_id: plugin_id.to_string(),
        manifest_version: v,
        builder_id: "builder/ci-release".to_string(),
        sbom_reference: "sbom/plugin.cdx.json".to_string(),
    }
}

fn provenance_json(plugin_id: &str, v: SemVer, key_ref: &str, artifact: &[u8]) -> String {
    let record = provenance(plugin_id, v, key_ref, artifact);
    serde_json::json!({
        "provenance-schema": PROVENANCE_SCHEMA_ID,
        "artifact-id": record.artifact_id,
        "digest-algorithm": record.digest_algorithm,
        "digest": record.digest_hex,
        "signer-key-reference": record.signer_key_reference,
        "signed-at-millis": record.signed_at_millis,
        "manifest-id": record.manifest_id,
        "manifest-version": record.manifest_version.to_string(),
        "builder-id": record.builder_id,
        "sbom-reference": record.sbom_reference,
    })
    .to_string()
}

/// Drives a lifecycle from registration to Ready (the P7.07 helper
/// pattern).
fn drive_to_ready(lifecycle: &mut PluginLifecycle) {
    lifecycle
        .verify(GateOutcome::Passed, "test", "provenance accepted")
        .expect("verify");
    lifecycle
        .mark_compatible(GateOutcome::Passed, "test", "compatible")
        .expect("compatible");
    lifecycle
        .configure(GateOutcome::Passed, "test", "configured")
        .expect("configure");
    lifecycle
        .start(GateOutcome::Passed, "test", "started")
        .expect("start");
    lifecycle
        .mark_ready(GateOutcome::Passed, "test", "ready")
        .expect("ready");
}

// --- 1. Revoke-by-id blocks verify + start --------------------------------------

#[test]
fn a_revoked_plugin_is_refused_at_the_verified_gate_before_any_provenance() {
    let clock = FixedClock::new(BASE_MS);
    let register = MemoryRevocationRegister::new();
    let audit = MemoryRevocationAuditSink::new();
    let v = version(0, 1, 0);

    revoke_plugin(
        &register,
        PLUGIN_ID,
        v,
        "incident-4711: malicious build",
        "security-operator",
        &clock,
        &audit,
    )
    .expect("revocation recorded");

    // The `Discovered → Verified` gate consults revocation FIRST — the
    // provenance digest would verify, and the plugin is still refused
    // (see the gate-order test below for the composed proof).
    let gate = revocation_gate(&register, PLUGIN_ID, v, Some(KEY_REF), &clock, &audit);
    assert!(
        !gate.passed(),
        "a revoked plugin must not pass the Verified gate"
    );
    let reason = gate.failure_reason().expect("a refusal names its reason");
    assert!(reason.contains("REVOKED_PLUGIN"), "the reason names the tag: {reason}");

    // The lifecycle refuses the transition fail-closed; the state stays
    // Discovered.
    let lifecycle_audit = MemoryAuditSink::new();
    let mut lifecycle = PluginLifecycle::register(
        PLUGIN_ID,
        &v.to_string(),
        &clock,
        &lifecycle_audit,
        DrainPolicy::default(),
    );
    lifecycle
        .verify(gate, "composition-root", "verify after revocation")
        .expect_err("the verify transition is refused");
    assert_eq!(lifecycle.state(), LifecycleState::Discovered);

    // The refusal is audited on the revocation audit port.
    let refused = audit
        .records()
        .iter()
        .find(|r| r.kind == RevocationAuditKind::RevocationCheckRefused)
        .expect("the gate refusal is audited");
    assert!(refused.detail.contains("REVOKED_PLUGIN"));

    // The register consult: exactly one record, keyed by id+version.
    let records = register.records();
    assert_eq!(records.len(), 1);
    assert_eq!(
        records[0].key,
        RevocationKey::Plugin { plugin_id: PLUGIN_ID.to_string(), version: v }
    );
}

#[test]
fn a_revoked_plugin_cannot_start_and_the_registry_time_recheck_refuses() {
    let clock = FixedClock::new(BASE_MS);
    let register = MemoryRevocationRegister::new();
    let audit = MemoryRevocationAuditSink::new();
    let v = version(0, 2, 0);

    // Verified FIRST, revoked AFTER — the running-plugin scenario.
    let lifecycle_audit = MemoryAuditSink::new();
    let mut lifecycle = PluginLifecycle::register(
        PLUGIN_ID,
        &v.to_string(),
        &clock,
        &lifecycle_audit,
        DrainPolicy::default(),
    );
    lifecycle
        .verify(GateOutcome::Passed, "test", "provenance accepted")
        .expect("verify");
    lifecycle
        .mark_compatible(GateOutcome::Passed, "test", "compatible")
        .expect("compatible");
    lifecycle
        .configure(GateOutcome::Passed, "test", "configured")
        .expect("configure");

    revoke_plugin(
        &register,
        PLUGIN_ID,
        v,
        "signature revoked post-verification",
        "security-operator",
        &clock,
        &audit,
    )
    .expect("revocation recorded");

    // The registry-time re-check refuses Start.
    let err = ensure_startable(&register, PLUGIN_ID, v, Some(KEY_REF), &clock, &audit)
        .expect_err("a revoked plugin cannot start");
    assert_eq!(err.name(), "PLUGIN_REVOKED");
    lifecycle
        .start(
            GateOutcome::Failed { reason: err.to_string() },
            "composition-root",
            "start after revocation",
        )
        .expect_err("the start gate refuses");
    assert_eq!(lifecycle.state(), LifecycleState::Compatible);

    // The revoked plugin cannot re-verify either (an explicit restart
    // lands back at Discovered, and the gate consults the register
    // again — same refusal, no path around it).
    let gate = revocation_gate(&register, PLUGIN_ID, v, Some(KEY_REF), &clock, &audit);
    assert!(!gate.passed());
}

// --- 2. Revoke-by-key blocks everything it signed ---------------------------------

#[test]
fn revoking_a_signer_key_blocks_every_plugin_it_signed_and_only_those() {
    let clock = FixedClock::new(BASE_MS);
    let register = MemoryRevocationRegister::new();
    let audit = MemoryRevocationAuditSink::new();
    let v = version(0, 3, 0);

    revoke_signer_key(
        &register,
        KEY_REF,
        "key compromise: CRL 2026-11-13",
        "security-operator",
        &clock,
        &audit,
    )
    .expect("key revocation recorded");

    // Plugin A names the revoked key — refused at the verify gate.
    let gate_a = revocation_gate(&register, "neryva.plugin-a", v, Some(KEY_REF), &clock, &audit);
    assert!(!gate_a.passed());
    assert!(gate_a
        .failure_reason()
        .expect("reason")
        .contains("REVOKED_SIGNER_KEY"));

    // Plugin B (same revoked key) — refused too: revoking a key revokes
    // everything it signed.
    let gate_b = revocation_gate(&register, "neryva.plugin-b", v, Some(KEY_REF), &clock, &audit);
    assert!(!gate_b.passed());

    // Plugin C (a different key) — NOT refused by the key revocation.
    let gate_c = revocation_gate(
        &register,
        "neryva.plugin-c",
        v,
        Some(OTHER_KEY_REF),
        &clock,
        &audit,
    );
    assert!(gate_c.passed());

    // The start re-check for a key-revoked plugin refuses as well.
    let err = ensure_startable(&register, "neryva.plugin-a", v, Some(KEY_REF), &clock, &audit)
        .expect_err("key-revoked plugin cannot start");
    assert_eq!(err.name(), "PLUGIN_REVOKED");
}

// --- 3. Append-only register (no un-revoke) ---------------------------------------

#[test]
fn the_register_is_append_only_and_un_revocation_is_a_recorded_not_implemented_seam() {
    let clock = FixedClock::new(BASE_MS);
    let register = MemoryRevocationRegister::new();
    let audit = MemoryRevocationAuditSink::new();
    let v = version(1, 0, 0);

    revoke_plugin(&register, PLUGIN_ID, v, "first revocation", "op", &clock, &audit)
        .expect("first revocation recorded");

    // A duplicate revocation is REFUSED, never a silent rewrite — the
    // register keeps exactly one record per key.
    let err = revoke_plugin(&register, PLUGIN_ID, v, "duplicate", "op", &clock, &audit)
        .expect_err("duplicate revocation refused");
    assert_eq!(err.name(), "REVOCATION_ALREADY_REVOKED");
    assert_eq!(register.records().len(), 1);

    // The seam: un-revocation is a NEW SIGNED RECORD TYPE, not an API.
    // The trait surface is the proof — `RevocationStore` has `append`
    // and `load_all` and NOTHING else; there is no remove/override
    // method to call. The statement constants pin the seam for review.
    assert!(REVOCATION_STATEMENT.contains("APPEND-ONLY"));
    assert!(UNREVOCATION_SEAM.contains("no removal or override API"));
    assert!(UNREVOCATION_SEAM.contains("NEW signed record type"));

    // Revocations require reason AND actor — nothing unattributed is
    // ever recorded.
    assert_eq!(
        revoke_plugin(&register, "neryva.other", v, "  ", "op", &clock, &audit)
            .expect_err("empty reason refused")
            .name(),
        "REVOCATION_REASON_REQUIRED"
    );
    assert_eq!(
        revoke_plugin(&register, "neryva.other", v, "reason", " ", &clock, &audit)
            .expect_err("empty actor refused")
            .name(),
        "REVOCATION_ACTOR_REQUIRED"
    );
    assert_eq!(register.records().len(), 1, "nothing unattributed was recorded");

    // A fail-closed check: a register that cannot be read never reports
    // "not revoked".
    struct BrokenRegister;
    impl RevocationStore for BrokenRegister {
        fn append(&self, _record: &RevocationRecord) -> Result<(), RevocationError> {
            Err(RevocationError::RegisterUnavailable {
                reason: "broken".to_string(),
            })
        }
        fn load_all(&self) -> Result<Vec<RevocationRecord>, RevocationError> {
            Err(RevocationError::RegisterUnavailable {
                reason: "broken".to_string(),
            })
        }
    }
    let broken = BrokenRegister;
    let err = is_revoked(&broken, &RevocationKey::SignerKey {
        key_reference: KEY_REF.to_string(),
    })
    .expect_err("a broken register is a check failure");
    assert_eq!(err.name(), "REVOCATION_CHECK_FAILED");
}

// --- 4. Staged rollout --------------------------------------------------------------

#[test]
fn staged_promotion_follows_the_declared_order_and_out_of_order_promotion_is_refused() {
    let clock = FixedClock::new(BASE_MS);
    let audit = MemoryRevocationAuditSink::new();
    let plan = StagedRollout::plan(
        PLUGIN_ID,
        version(0, 4, 0),
        version(0, 5, 0),
        vec![
            "sim-lab".to_string(),
            "vpp-operator".to_string(),
            "site-edge".to_string(),
        ],
    )
    .expect("plan");

    // THE NO-FLAG-FLIP REFUSAL: promoting the LAST profile first (a full
    // rollout in one step) is refused — only the exact next un-promoted
    // profile may be promoted.
    let err = plan
        .clone()
        .promote("site-edge", "op", "jump to the end", &clock, &audit)
        .expect_err("out-of-order promotion refused");
    match err {
        RolloutError::PromotionOutOfOrder { profile_id, expected_next } => {
            assert_eq!(profile_id, "site-edge");
            assert_eq!(expected_next, "sim-lab");
        }
        other => panic!("unexpected error: {other:?}"),
    }

    // An unknown profile is refused.
    let mut plan = plan;
    assert_eq!(
        plan.promote("research-gnn", "op", "not in the plan", &clock, &audit)
            .expect_err("unknown profile refused")
            .name(),
        "PROMOTION_UNKNOWN_PROFILE"
    );

    // The happy path: one profile at a time, in the declared order.
    plan.promote("sim-lab", "op", "stage 1: lab only", &clock, &audit)
        .expect("stage 1");
    assert_eq!(plan.promoted(), ["sim-lab"]);
    plan.promote("vpp-operator", "op", "stage 2", &clock, &audit)
        .expect("stage 2");
    assert!(!plan.is_complete());
    plan.promote("site-edge", "op", "stage 3", &clock, &audit)
        .expect("stage 3");
    assert!(plan.is_complete());
    assert!(plan.next_profile().is_none());

    // Nothing promotes past completion.
    assert_eq!(
        plan.promote("site-edge", "op", "again", &clock, &audit)
            .expect_err("complete rollout cannot promote again")
            .name(),
        "PROMOTION_OUT_OF_ORDER"
    );

    // Every promotion (and every refusal) is audited.
    let promoted = audit
        .records()
        .iter()
        .filter(|r| r.kind == RevocationAuditKind::PromotionRecorded)
        .count();
    assert_eq!(promoted, 3);
    let refused = audit
        .records()
        .iter()
        .filter(|r| r.kind == RevocationAuditKind::PromotionRefused)
        .count();
    assert_eq!(refused, 3);

    // The recorded model: NO percentage canary anywhere on the type
    // surface (the statement is pinned; there is no cohort/percentage
    // field or method to call).
    assert!(ROLLOUT_STATEMENT.contains("NO percentage canary"));
    assert!(ROLLOUT_STATEMENT.contains("ONE profile first"));

    // Plan-shape refusals.
    assert_eq!(
        StagedRollout::plan(
            PLUGIN_ID,
            version(0, 5, 0),
            version(0, 5, 0),
            vec!["sim-lab".to_string()],
        )
        .expect_err("no-op rollout refused")
        .name(),
        "ROLLOUT_NO_OP"
    );
    assert_eq!(
        StagedRollout::plan(PLUGIN_ID, version(0, 5, 0), version(0, 6, 0), Vec::new())
            .expect_err("empty plan refused")
            .name(),
        "ROLLOUT_PLAN_INVALID"
    );
}

// --- 5/6. Explicit removal with audit; audit trail preserved ------------------------

#[test]
fn a_failed_plugin_is_removed_explicitly_with_drain_and_a_full_audited_trail() {
    let clock = FixedClock::new(BASE_MS);
    let revocation_audit = MemoryRevocationAuditSink::new();

    let lifecycle_audit = MemoryAuditSink::new();
    let mut lifecycle = PluginLifecycle::register(
        PLUGIN_ID,
        &version(0, 6, 0).to_string(),
        &clock,
        &lifecycle_audit,
        DrainPolicy::default(),
    );
    drive_to_ready(&mut lifecycle);
    lifecycle
        .fail("test", "runtime failure: repeated health-check timeout")
        .expect("failed");
    assert_eq!(lifecycle.state(), LifecycleState::Failed);

    // Removal is EXPLICIT (it does not happen as a side effect of
    // failing): until it is requested, the plugin holds Failed.
    assert_eq!(lifecycle.state(), LifecycleState::Failed);

    // A Failed plugin with in-flight records is NOT silently dropped:
    // the drain must quiesce first.
    let mut drain = TestDrain::with_work(3);
    let outcome = remove_failed_plugin(
        &mut lifecycle,
        &mut drain,
        "P7.30 drill: failed plugin removal",
        "operator",
        &clock,
        &revocation_audit,
    )
    .expect("removal");
    assert_eq!(outcome, DrainOutcome::Quiesced { cancelled: 3 });
    assert_eq!(lifecycle.state(), LifecycleState::Stopped);
    assert_eq!(drain.outstanding_work(), 0, "no residual scheduled work remains");

    // Every step is audited: requested + completed.
    let kinds: Vec<RevocationAuditKind> =
        revocation_audit.records().iter().map(|r| r.kind).collect();
    assert!(kinds.contains(&RevocationAuditKind::RemovalRequested));
    assert!(kinds.contains(&RevocationAuditKind::RemovalCompleted));
    let completed = revocation_audit
        .records()
        .iter()
        .find(|r| r.kind == RevocationAuditKind::RemovalCompleted)
        .expect("completion record");
    assert!(
        completed.detail.contains("audit trail is PRESERVED"),
        "the completion record states the trail preservation"
    );

    // THE TRAIL IS PRESERVED: the lifecycle's records (registration,
    // every transition, the failure, the drain) are all still in the
    // audit sink — removal deleted nothing.
    let trail = lifecycle_audit.records();
    assert!(trail.len() >= 8, "the full trail survives: {} records", trail.len());
    assert!(trail
        .iter()
        .any(|r| r.kind.name() == "REGISTRATION"));
    assert!(trail
        .iter()
        .any(|r| r.from == Some(LifecycleState::Ready) && r.to == Some(LifecycleState::Failed)));
    assert!(trail
        .iter()
        .any(|r| r.from == Some(LifecycleState::Failed) && r.to == Some(LifecycleState::Draining)));
    assert!(trail
        .iter()
        .any(|r| r.from == Some(LifecycleState::Draining) && r.to == Some(LifecycleState::Stopped)));

    assert!(REMOVAL_STATEMENT.contains("NEVER deletes the plugin's audit trail"));
}

// --- 7. Removal refusals ------------------------------------------------------------

#[test]
fn removal_is_refused_for_a_running_plugin_and_for_an_undrained_failed_plugin() {
    let clock = FixedClock::new(BASE_MS);
    let audit = MemoryRevocationAuditSink::new();

    // A RUNNING (Ready) plugin is not removable through this path.
    let lifecycle_audit = MemoryAuditSink::new();
    let mut lifecycle = PluginLifecycle::register(
        PLUGIN_ID,
        &version(0, 7, 0).to_string(),
        &clock,
        &lifecycle_audit,
        DrainPolicy::default(),
    );
    drive_to_ready(&mut lifecycle);
    let mut drain = TestDrain::with_work(0);
    let err = remove_failed_plugin(
        &mut lifecycle,
        &mut drain,
        "try to remove a running plugin",
        "operator",
        &clock,
        &audit,
    )
    .expect_err("removal refused from Ready");
    match err {
        RemovalError::RequiresFailedState { state } => assert_eq!(state, LifecycleState::Ready),
        other => panic!("unexpected error: {other:?}"),
    }
    assert_eq!(lifecycle.state(), LifecycleState::Ready, "nothing changed");

    // A Failed plugin with UNDRAINED work past the deadline: the
    // lifecycle's fail-closed rule restores it to Ready (no forced stop),
    // the removal is refused, and the refusal is audited with the
    // containment instruction.
    let lifecycle_audit = MemoryAuditSink::new();
    let mut stuck = PluginLifecycle::register(
        "neryva.malicious-plugin",
        &version(9, 9, 9).to_string(),
        &clock,
        &lifecycle_audit,
        DrainPolicy::default(),
    );
    drive_to_ready(&mut stuck);
    stuck.fail("test", "compromised: P7.29 scenario").expect("failed");
    let mut drain = TestDrain::stuck(5);
    let err = remove_failed_plugin(
        &mut stuck,
        &mut drain,
        "P7.29 malicious plugin removal",
        "operator",
        &clock,
        &audit,
    )
    .expect_err("undrained plugin not removable");
    assert_eq!(err.name(), "REMOVAL_DRAIN_DEADLINE_EXCEEDED");
    // The lifecycle restored the plugin to Ready (its recorded rule);
    // containment now goes through revocation / grant drops, not a
    // forced stop.
    assert_eq!(stuck.state(), LifecycleState::Ready);
    let refused = audit
        .records()
        .iter()
        .find(|r| r.kind == RevocationAuditKind::RemovalRefused)
        .expect("the removal refusal is audited");
    assert!(refused.detail.contains("contain the plugin"));

    // Unattributed removals are refused.
    let lifecycle_audit = MemoryAuditSink::new();
    let mut lifecycle = PluginLifecycle::register(
        "neryva.another",
        &version(0, 1, 0).to_string(),
        &clock,
        &lifecycle_audit,
        DrainPolicy::default(),
    );
    lifecycle.crash("test", "crash in Discovered").expect("crash");
    let mut drain = TestDrain::with_work(0);
    assert_eq!(
        remove_failed_plugin(&mut lifecycle, &mut drain, " ", "operator", &clock, &audit)
            .expect_err("empty reason refused")
            .name(),
        "REMOVAL_REASON_REQUIRED"
    );
    assert_eq!(
        remove_failed_plugin(&mut lifecycle, &mut drain, "reason", "", &clock, &audit)
            .expect_err("empty actor refused")
            .name(),
        "REMOVAL_ACTOR_REQUIRED"
    );
}

// --- 8. The file-backed register ------------------------------------------------------

#[test]
fn the_file_register_survives_reopen_and_refuses_a_foreign_or_corrupt_file() {
    let clock = FixedClock::new(BASE_MS);
    let audit = MemoryRevocationAuditSink::new();
    let path = scratch_path("register");

    // Revoke two keys, drop the register.
    {
        let register = FileRevocationRegister::open(&path).expect("open");
        assert_eq!(register.corrupt_tail_truncated(), 0);
        revoke_plugin(
            &register,
            PLUGIN_ID,
            version(1, 1, 0),
            "incident",
            "op",
            &clock,
            &audit,
        )
        .expect("recorded");
        revoke_signer_key(&register, KEY_REF, "key compromise", "op", &clock, &audit)
            .expect("recorded");
        assert_eq!(register.records_written(), 2);
    }

    // Reopen: both records survive (append-only journal, last wins per
    // key; the replay order is the append order).
    let reopened = FileRevocationRegister::open(&path).expect("reopen");
    let records = reopened.load_all().expect("load");
    assert_eq!(records.len(), 2);
    assert_eq!(records[0].key.register_key(), format!("plugin/{PLUGIN_ID}@1.1.0"));
    assert_eq!(records[1].key.register_key(), format!("signer/{KEY_REF}"));

    // A duplicate through the file register is refused too.
    let err = revoke_plugin(
        &reopened,
        PLUGIN_ID,
        version(1, 1, 0),
        "again",
        "op",
        &clock,
        &audit,
    )
    .expect_err("duplicate refused");
    assert_eq!(err.name(), "REVOCATION_ALREADY_REVOKED");

    // A foreign file is refused at open, never guessed at.
    let foreign_path = scratch_path("foreign");
    std::fs::write(&foreign_path, b"not a flexcore register at all").expect("write");
    let err = FileRevocationRegister::open(&foreign_path).expect_err("foreign file refused");
    assert_eq!(err.name(), "REVOCATION_REGISTER_FILE_FOREIGN");

    // A corrupt tail is truncated and COUNTED, never silent: a good
    // record, then garbage appended OUTSIDE the register (dropped first —
    // the register's own file handle is private by design).
    let corrupt_path = scratch_path("corrupt");
    {
        let register = FileRevocationRegister::open(&corrupt_path).expect("open");
        revoke_plugin(
            &register,
            "neryva.good-plugin",
            version(0, 1, 0),
            "good record",
            "op",
            &clock,
            &audit,
        )
        .expect("recorded");
    }
    {
        use std::io::Write;
        let mut file = std::fs::OpenOptions::new()
            .append(true)
            .open(&corrupt_path)
            .expect("append");
        file.write_all(&[0xDE, 0xAD, 0xBE, 0xEF]).expect("corrupt tail");
        file.sync_all().expect("sync");
    }
    let recovered = FileRevocationRegister::open(&corrupt_path).expect("recover");
    assert_eq!(recovered.corrupt_tail_truncated(), 1, "the loss is counted");
    let records = recovered.load_all().expect("load");
    assert_eq!(records.len(), 1, "the good record survives");
    assert_eq!(records[0].key.register_key(), "plugin/neryva.good-plugin@0.1.0");
}

// --- 9. Gate order: revocation BEFORE provenance ---------------------------------------

#[test]
fn the_verified_gate_order_is_revocation_first_then_provenance() {
    let clock = FixedClock::new(BASE_MS);
    let register = MemoryRevocationRegister::new();
    let audit = MemoryRevocationAuditSink::new();
    let v = version(0, 8, 0);
    let artifact = b"artifact bytes";

    // The provenance record is VALID (the digest matches, the binding
    // matches) — yet a revoked plugin must not pass.
    let json = provenance_json(PLUGIN_ID, v, KEY_REF, artifact);
    let record = flexcore_kernel::signing::parse_provenance(&json).expect("valid record");
    let provenance_outcome = provenance_gate(
        Some(&record),
        artifact,
        PLUGIN_ID,
        v,
        &UnsafeTestOnlyDigest,
        &clock,
        &MemoryAuditSink::new(),
    );
    assert!(provenance_outcome.passed(), "fixture sanity: provenance alone verifies");

    // Revoke, then compose the gates in the P7.38 order.
    revoke_plugin(
        &register,
        PLUGIN_ID,
        v,
        "revoked after signing",
        "security-operator",
        &clock,
        &audit,
    )
    .expect("recorded");
    let revocation_outcome =
        revocation_gate(&register, PLUGIN_ID, v, Some(KEY_REF), &clock, &audit);
    assert!(!revocation_outcome.passed());
    // The caller MUST refuse at revocation and never reach the
    // provenance gate: the composed outcome is the revocation refusal.
    let composed = match revocation_outcome {
        GateOutcome::Passed => provenance_outcome,
        failed @ GateOutcome::Failed { .. } => failed,
    };
    assert!(composed
        .failure_reason()
        .expect("reason")
        .contains("REVOKED_PLUGIN"));

    // And for a NON-revoked plugin the composed order passes only when
    // BOTH gates pass (the register consult adds a refusal class, never
    // an acceptance class).
    let other = "neryva.clean-plugin";
    assert!(revocation_gate(&register, other, v, Some(KEY_REF), &clock, &audit).passed());
}

// --- 10. Error-name hygiene (house style) -----------------------------------------------

#[test]
fn revocation_and_removal_error_names_are_pairwise_distinct() {
    let names = [
        RevocationError::CheckFailed { reason: String::new() }.name(),
        RevocationError::RegisterUnavailable { reason: String::new() }.name(),
        RevocationError::RegisterFileForeign { reason: String::new() }.name(),
        RevocationError::AlreadyRevoked { key: String::new() }.name(),
        RevocationError::Revoked { key: String::new(), revocation_reason: String::new() }.name(),
        RevocationError::KeyInvalid { reason: String::new() }.name(),
        RevocationError::ReasonRequired.name(),
        RevocationError::ActorRequired.name(),
        RolloutError::PlanInvalid { reason: String::new() }.name(),
        RolloutError::NoOpRollout { version: String::new() }.name(),
        RolloutError::PromotionUnknownProfile { profile_id: String::new() }.name(),
        RolloutError::PromotionOutOfOrder {
            profile_id: String::new(),
            expected_next: String::new(),
        }
        .name(),
        RolloutError::ReasonRequired.name(),
        RolloutError::ActorRequired.name(),
        RemovalError::RequiresFailedState { state: LifecycleState::Ready }.name(),
        RemovalError::ReasonRequired.name(),
        RemovalError::ActorRequired.name(),
        RemovalError::DrainDeadlineExceeded { outstanding: 0 }.name(),
        RemovalError::LifecycleRefused {
            source: flexcore_kernel::lifecycle::LifecycleError::DrainPortMissing,
        }
        .name(),
    ];
    let mut sorted = names.to_vec();
    sorted.sort();
    let count = sorted.len();
    sorted.dedup();
    assert_eq!(sorted.len(), count, "error names must be pairwise distinct");

    // The revocation audit-kind names are distinct too.
    let audit_names = [
        RevocationAuditKind::RevocationAppended.name(),
        RevocationAuditKind::RevocationRefused.name(),
        RevocationAuditKind::RevocationCheckRefused.name(),
        RevocationAuditKind::PromotionRecorded.name(),
        RevocationAuditKind::PromotionRefused.name(),
        RevocationAuditKind::RemovalRequested.name(),
        RevocationAuditKind::RemovalCompleted.name(),
        RevocationAuditKind::RemovalRefused.name(),
    ];
    let mut sorted_audit = audit_names.to_vec();
    sorted_audit.sort();
    let audit_count = sorted_audit.len();
    sorted_audit.dedup();
    assert_eq!(sorted_audit.len(), audit_count);
}
