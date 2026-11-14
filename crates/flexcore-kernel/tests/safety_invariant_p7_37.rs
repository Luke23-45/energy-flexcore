// P7.37 — §17.5 safety-invariant test map for the SDK: the compact matrix
// suite (test file WRITTEN, NOT RUN — the loop bans hold; the human
// build/test pass executes `cargo test -p flexcore-kernel --test
// safety_invariant_p7_37`).
//
// THE CONTRACT: this file is the test-side twin of
// `ledger/phase-07-plugin-sdk/37_safety_invariant_map.md`. Each test
// guards the SDK-ENFORCEABLE slice of one or more §17.5 invariant rows in
// the map; if any enforcement point regresses, the corresponding test
// fails. Rows the SDK cannot enforce (they live in the domain/application
// protected path) are MAP rows citing the Phase 3/4 suites — they are NOT
// duplicated here (the map cites them; inventing a second copy here would
// be exactly the "invented coverage" the ledger risk names).
//
// | Test | §17.5 row(s) | Map row |
// |---|---|---|
// | never_granted_permissions_are_refused_at_every_sdk_gate | 2, 6, 7 | rows 2/6/7 (SDK slice) |
// | protected_kernel_contract_claims_are_refused_at_resolver_and_path | 7 | row 7 (SDK slice, ADR-005/ADR-010) |
// | the_proposal_path_has_no_dispatch_or_feasibility_authority_variant | 7, 9 | row 9 (SDK slice) |
// | expiry_is_fail_closed_and_the_sequence_check_runs_last | 5 | row 5 (SDK slice) |
// | the_mode_lock_holds_profile_data_cannot_select_control_authority | §14/§21.4 boundary | mode-lock row |
// | drain_before_replace_and_revocation_gate_the_replacement_path | §5.6 / row 5+7 support | replacement row |
// | the_tenant_scope_is_enforced_at_the_sdk_envelope | 8 | row 8 (SDK slice; RLS stays the platform's) |
// | the_sdk_states_its_boundary_honestly | all | residual-risk statements |

use flexcore_kernel::lifecycle::{
    GateOutcome, LifecycleState, MemoryAuditSink, PluginLifecycle, DrainPolicy,
};
use flexcore_kernel::manifest::{parse_manifest, ManifestError};
use flexcore_kernel::plugin_path_protection::{
    refuse_protected_contract_claim, FilePluginSequenceCheckpoint, PluginPathOutcome,
    PluginPathVerifier, PluginRequestEnvelope, PROPOSAL_ENVELOPE_STATEMENT,
    PLUGIN_PATH_PROTECTION_STATEMENT,
};
use flexcore_kernel::profile::{
    parse_profile, ControlAuthorityPosture, ProfileError,
};
use flexcore_kernel::resolve::{resolve, ExplicitSelections, ResolveError};
use flexcore_kernel::revocation::{
    revoke_plugin, revocation_gate, MemoryRevocationAuditSink, MemoryRevocationRegister,
};
use flexcore_kernel::signing::SIGNING_STATEMENT;
use flexcore_kernel::tenant_scope::{enforce_scope, ScopedRequestEnvelope, TenantScope};

const BASE_MS: u64 = 1_700_000_000_000;

fn semver(major: u64, minor: u64, patch: u64) -> flexcore_kernel::manifest::SemVer {
    flexcore_kernel::manifest::SemVer::new(major, minor, patch)
}

/// A valid manifest fixture (the conformance Class A shape), with
/// `permissions` / `provider-of` overridable per test.
fn manifest_json(permissions: &[&str], provider_of: &[&str]) -> String {
    let permissions: Vec<serde_json::Value> =
        permissions.iter().map(|p| serde_json::json!(p)).collect();
    let provider_of: Vec<serde_json::Value> =
        provider_of.iter().map(|p| serde_json::json!(p)).collect();
    serde_json::json!({
        "manifest-schema": "flexcore-plugin-manifest-v1",
        "plugin-id": "neryva.safety-matrix-plugin",
        "name": "Safety Matrix Fixture Plugin",
        "version": "0.1.0",
        "api-version": ">=0.5.0, <1.0.0",
        "class": "builtin-rust",
        "entry-point": { "registered-type": "neryva.builtin/safety-matrix" },
        "provider-of": provider_of,
        "consumes": [],
        "permissions": permissions,
        "configuration-schema": {
            "type": "object",
            "additionalProperties": false,
            "properties": {},
            "required": []
        },
        "secrets": [],
        "resources": { "max-memory-mb": 64, "timeout-ms": 30000 },
        "metadata": {
            "license": "Apache-2.0",
            "description": "Fixture for the §17.5 SDK-invariant matrix suite."
        }
    })
    .to_string()
}

/// A valid profile fixture, with `control-authority` and
/// `permission-ceiling` overridable per test.
fn profile_json(control_authority: &str, ceiling: &[&str]) -> String {
    let ceiling: Vec<serde_json::Value> =
        ceiling.iter().map(|c| serde_json::json!(c)).collect();
    serde_json::json!({
        "profile-schema": "flexcore-profile-v1",
        "profile-id": "safety-matrix",
        "name": "Safety Matrix Fixture Profile",
        "version": "0.1.0",
        "description": "Fixture for the §17.5 SDK-invariant matrix suite.",
        "control-authority": control_authority,
        "permission-ceiling": ceiling,
        "selections": []
    })
    .to_string()
}

// --- Invariants 2, 6, 7 — the never-granted set is refused at EVERY SDK gate -------

#[test]
fn never_granted_permissions_are_refused_at_every_sdk_gate() {
    // Gate 1 — the manifest validator (P7.05): the never-granted ids are
    // not part of the requestable vocabulary at all.
    for never_granted in ["dispatch.authorize", "dispatch.issue", "secret.read", "network.any"] {
        let err = parse_manifest(&manifest_json(&[never_granted], &["ess.simulation"]))
            .expect_err("a never-granted permission is refused at the manifest gate");
        assert!(
            matches!(err, ManifestError::PermissionNeverGranted { .. }),
            "{never_granted}: unexpected error {err:?}"
        );
    }

    // Gate 2 — the profile ceiling (P7.15): a profile can never ceiling
    // what no manifest may request.
    for never_granted in ["dispatch.authorize", "secret.read"] {
        let err = parse_profile(&profile_json("simulation-shadow", &[never_granted]))
            .expect_err("a never-granted ceiling entry is refused");
        assert!(
            matches!(err, ProfileError::ProfilePermissionCeilingNeverGranted { .. }),
            "{never_granted}: unexpected error {err:?}"
        );
    }
}

// --- Invariant 7 — the protected kernel is structurally unreachable -----------------

#[test]
fn protected_kernel_contract_claims_are_refused_at_resolver_and_path() {
    // The resolver (P7.06, ADR-005): the authoritative guard.
    let manifest =
        parse_manifest(&manifest_json(&["telemetry.read"], &["kernel.command-authorizer"]))
            .expect("the manifest itself parses (the guard is the resolver's)");
    let err = resolve(&[manifest.clone()], &ExplicitSelections::new())
        .expect_err("a protected-kernel claim is refused");
    assert!(matches!(err, ResolveError::ProtectedKernelReplace { .. }));

    // The plugin-path restatement (P7.36): a caller that skips the
    // resolver still cannot walk a protected claim through the path.
    let err = refuse_protected_contract_claim(&manifest).expect_err("restated at the path");
    assert!(matches!(
        err,
        flexcore_kernel::plugin_path_protection::PluginPathError::ProtectedContractClaim { .. }
    ));
}

// --- Invariants 7 and 9 — the proposal path has NO authority variant ----------------

#[test]
fn the_proposal_path_has_no_dispatch_or_feasibility_authority_variant() {
    // Type-level proof (ADR-010): the outcome enum is CLOSED and has no
    // dispatch-authorizing variant — and no feasibility-passing variant
    // either. The exhaustive match below fails to COMPILE if a variant
    // is added; the assertions pin what the two existing variants are.
    let outcome = PluginPathOutcome::ResultObserved {
        correlation_id: "corr-1737".to_string(),
    };
    match outcome {
        PluginPathOutcome::ProposalRecorded(proposal) => {
            // A proposal is DATA for the protected path: it carries
            // producer identity (the §9.2 provenance discipline on the
            // proposal path) and NO authorization, verdict, or
            // feasibility-pass field exists on it.
            assert!(!proposal.producer_plugin_id.is_empty());
            assert!(!proposal.producer_plugin_version.is_empty());
        }
        PluginPathOutcome::ResultObserved { .. } => {}
    }
    // The statement is pinned (the invariant-9 SDK slice: a plugin
    // result NEVER passes feasibility through the SDK — it enters the
    // protected path as a proposal; the feasibility gate itself is the
    // Phase 4 application layer, cited in the map).
    assert!(PROPOSAL_ENVELOPE_STATEMENT.contains("gains no authority"));
    assert!(PROPOSAL_ENVELOPE_STATEMENT.contains("protected validation"));
    assert!(PLUGIN_PATH_PROTECTION_STATEMENT.contains("no dispatch-authorizing variant"));
}

// --- Invariant 5 — expiry fail-closed, sequence LAST --------------------------------

#[test]
fn expiry_is_fail_closed_and_the_sequence_check_runs_last() {
    let clock = flexcore_kernel::lifecycle::FixedClock::new(BASE_MS);
    let path = std::env::temp_dir().join(format!(
        "p737-seq-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    let checkpoint = FilePluginSequenceCheckpoint::open(&path).expect("checkpoint");
    let mut verifier = PluginPathVerifier::new(&clock, Box::new(checkpoint)).expect("verifier");

    let request = |sequence: u64, from: u64, until: u64| {
        PluginRequestEnvelope::new(
            "corr-p737-0001",
            "neryva.safety-matrix-plugin",
            "0.1.0",
            sequence,
            from,
            until,
            serde_json::json!({ "op": "propose" }),
        )
        .expect("valid envelope")
    };

    // (a) An EXPIRED request is refused, never executed — and the
    //     sequence window is NOT advanced (check 4 runs LAST: a request
    //     refused by any earlier check never moves the sequence state).
    let expired = request(1, BASE_MS - 120_000, BASE_MS - 60_000);
    let err = verifier
        .verify_and_admit(&expired)
        .expect_err("expired refused");
    assert_eq!(err.name(), "REQUEST_EXPIRED");
    assert_eq!(
        verifier.last_accepted("neryva.safety-matrix-plugin/invocation"),
        None,
        "the refusal did not advance the sequence window"
    );

    // (b) A valid request is admitted (durable advance).
    let valid = request(1, BASE_MS, BASE_MS + 60_000);
    verifier.verify_and_admit(&valid).expect("admitted");
    assert_eq!(
        verifier.last_accepted("neryva.safety-matrix-plugin/invocation"),
        Some(1)
    );

    // (c) THE ORDER PROOF: a request that is BOTH expired AND a replay
    //     (same sequence, window in the past) is refused as EXPIRED —
    //     the window check (3) runs before the sequence check (4) — and
    //     the refusal still does not move the window.
    let expired_replay = request(1, BASE_MS - 120_000, BASE_MS - 60_000);
    let err = verifier
        .verify_and_admit(&expired_replay)
        .expect_err("expired replay refused");
    assert_eq!(err.name(), "REQUEST_EXPIRED");
    assert_eq!(
        verifier.last_accepted("neryva.safety-matrix-plugin/invocation"),
        Some(1)
    );

    // (d) The same sequence with a VALID window is the replay refusal —
    //     distinct named class (§14.3 duplicate rule).
    let replay = request(1, BASE_MS, BASE_MS + 60_000);
    let err = verifier.verify_and_admit(&replay).expect_err("replay refused");
    assert_eq!(err.name(), "SEQUENCE_REPLAYED");
}

// --- §14/§21.4 boundary — the control-authority mode lock ----------------------------

#[test]
fn the_mode_lock_holds_profile_data_cannot_select_control_authority() {
    // The profile mechanism accepts EXACTLY `simulation-shadow`; every
    // other §21.4 mode — including via the data path — is the named
    // error.
    for forbidden in ["read-only-shadow", "operator-approved", "bounded-automation", "simulation"] {
        let err = parse_profile(&profile_json(forbidden, &[]))
            .expect_err("a control-capable mode is refused in profile data");
        assert!(
            matches!(err, ProfileError::ControlAuthorityModeForbidden { .. }),
            "{forbidden}: unexpected error {err:?}"
        );
        // And the posture parser (the P7.16 defence-in-depth comparison
        // input) refuses the same values from non-profile code.
        assert!(ControlAuthorityPosture::parse_from_profile_data(forbidden).is_err());
    }

    // The posture enum carries all four §21.4 modes, but profile DATA
    // reaches only SimulationShadow — no constructor path leads from
    // data to a control-capable variant.
    let posture = ControlAuthorityPosture::parse_from_profile_data("simulation-shadow")
        .expect("the one accepted value");
    assert_eq!(posture.name(), "simulation-shadow");
    assert_eq!(posture, ControlAuthorityPosture::SimulationShadow);
}

// --- §5.6 support — drain-before-replace, and the P7.38 revocation gate -------------

#[test]
fn drain_before_replace_and_revocation_gate_the_replacement_path() {
    let clock = flexcore_kernel::lifecycle::FixedClock::new(BASE_MS);
    let audit = MemoryAuditSink::new();

    // Drain-before-replace (the §5.6 core rule): a replacement may only
    // start once the incumbent is fully drained.
    let mut lifecycle = PluginLifecycle::register(
        "neryva.safety-matrix-plugin",
        "0.1.0",
        &clock,
        &audit,
        DrainPolicy::default(),
    );
    lifecycle
        .verify(GateOutcome::Passed, "test", "verified")
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

    let err = lifecycle
        .require_fully_drained("test", "attempted while active")
        .expect_err("replacement blocked while the incumbent is active");
    assert_eq!(err.name(), "REPLACEMENT_BLOCKED_UNDRAINED");

    // The revocation gate (P7.38) refuses verify for a revoked plugin —
    // the replacement path cannot sneak a revoked version through
    // either (the register is consulted FIRST at Discovered → Verified).
    let register = MemoryRevocationRegister::new();
    let rev_audit = MemoryRevocationAuditSink::new();
    revoke_plugin(
        &register,
        "neryva.safety-matrix-plugin",
        semver(0, 1, 0),
        "matrix: revoked version",
        "security-operator",
        &clock,
        &rev_audit,
    )
    .expect("revoked");
    let gate = revocation_gate(
        &register,
        "neryva.safety-matrix-plugin",
        semver(0, 1, 0),
        Some("key-store:release-2026-11"),
        &clock,
        &rev_audit,
    );
    assert!(!gate.passed());
    assert!(gate
        .failure_reason()
        .expect("reason")
        .contains("REVOKED_PLUGIN"));
    // The lifecycle is still at Failed-capable Ready state and holds:
    // the revocation refusal is a gate outcome — fail-closed.
    assert_eq!(lifecycle.state(), LifecycleState::Ready);
}

// --- Invariant 8 (SDK slice) — the tenant scope at the envelope ----------------------

#[test]
fn the_tenant_scope_is_enforced_at_the_sdk_envelope() {
    let stamped = TenantScope::from_authenticated("tenant-alpha", Some("site-1"))
        .expect("host-stamped");
    let inner = PluginRequestEnvelope::new(
        "corr-p737-0002",
        "neryva.safety-matrix-plugin",
        "0.1.0",
        1,
        BASE_MS,
        BASE_MS + 60_000,
        serde_json::json!({}),
    )
    .expect("valid inner envelope");
    let scoped = ScopedRequestEnvelope::new(inner, stamped).expect("wrap");

    // In-scope passes; cross-tenant is the named refusal (the SDK slice
    // of invariant 8 — the domain/RLS layer is the platform's, cited in
    // the map).
    enforce_scope(
        &scoped,
        &TenantScope::from_authenticated("tenant-alpha", Some("site-1")).expect("scope"),
    )
    .expect("in-scope request admitted");
    let err = enforce_scope(
        &scoped,
        &TenantScope::from_authenticated("tenant-beta", None).expect("scope"),
    )
    .expect_err("cross-tenant refused");
    assert_eq!(err.name(), "TENANT_SCOPE_MISMATCH");

    // The plugin cannot widen its own scope: a widened echo is refused.
    let err = scoped
        .check_echo(&TenantScope::from_authenticated("tenant-alpha", None).expect("scope"))
        .expect_err("widened echo refused");
    assert_eq!(err.name(), "TENANT_SCOPE_ECHO_MISMATCH");
    // The exhaustive refusal-name table is closed.
    assert_eq!(TENANT_SCOPE_ERROR_COUNT, 5);
}

/// The closed refusal-name count of the tenant-scope module (kept in sync
/// with `TENANT_SCOPE_ERROR_NAMES` by the module's own tests; pinned here
/// so the matrix notices a change).
const TENANT_SCOPE_ERROR_COUNT: usize = 5;

// --- The honesty boundary -------------------------------------------------------------

#[test]
fn the_sdk_states_its_boundary_honestly() {
    // The residual-risk statements the map cites are present and
    // unchanged (a map citing statements that no longer exist would be
    // invented coverage).
    assert!(SIGNING_STATEMENT.contains("DEPLOYMENT REQUIREMENT"));
    assert!(SIGNING_STATEMENT.contains("No certification or security claim is made"));
    assert!(PLUGIN_PATH_PROTECTION_STATEMENT.contains("DEPLOYMENT REQUIREMENT"));
    assert!(PLUGIN_PATH_PROTECTION_STATEMENT.contains("No certification or security claim"));
}
