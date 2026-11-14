// P7.29 — Security testing at the SDK boundary, KERNEL-side half: the
//         supply-chain attack chains (P7.14 provenance gate), the profile
//         downgrade substitution, the unprovenanced-plugin refusal, the
//         lifecycle-state abuse (restart policy, drain-deadline abuse), and
//         the forged-profile escalation refusals (report §19.2(7), §17.4,
//         §9.3, §5.6, §5.8; ledger `ledger_phase_7.md` §P7.29).
//         WRITTEN-UNRUN: the loop bans hold (no cargo command of any kind);
//         the first human `cargo test -p flexcore-kernel` run is the
//         verification gate. No pass is claimed anywhere.
//
// WHY THIS FILE EXISTS (the split discipline): P7.29's mandated supply-chain
// cases drive the KERNEL signing/profile/lifecycle types read-only, and
// `flexcore-plugin-host` depends on no flexcore crate (§8.2 dependency rule,
// enforced by `tools/check_dependency_rule.py`) — so the host-side test file
// `apps/plugin-host/tests/security_p7_29.rs` cannot reach them. This
// companion file holds the kernel-reachable attack chains; the host file
// holds the escalation/containment chains over the broker and enforcement
// door. The reuse-by-citation map in the dated report
// (`ledger/phase-07-plugin-sdk/29_security_test_report.md`) allocates every
// §19.2(7) category to exactly one owning suite; this file EXTENDS — never
// re-asserts — the unit cases P7.14 (`tests/signing_p7_14.rs`), P7.15
// (`tests/profile_p7_15.rs`), and P7.07 (`tests/lifecycle_p7_07.rs`) already
// own, by composing them into attempt → refusal → containment chains.
//
// CASE → REQUIREMENT MAP (the P6.19 pattern):
//
// | # | Case | Requirement |
// |---|---|---|
// | 1 | a_tampered_artifact_is_refused_at_reverification_and_never_advances | §19.2(7) supply chain (tampered package); §9.3; §17.4 (attempt → refusal → audit; the §5.6 default: a refused verify never advances) |
// | 2 | a_version_downgrade_substitution_is_refused_by_the_profile_constraint_and_the_provenance_binding | §19.2(7) supply chain (manifest/artifact substitution, version downgrade); §5.8 (profile version constraints); §9.3 (record binds the manifest) |
// | 3 | an_unprovenanced_plugin_is_refused_before_any_compatibility_check | §19.2(7) supply chain (no SBOM / no provenance); §9.3 (SBOM reference REQUIRED); the P7.14 before-compatibility rule |
// | 4 | a_restart_without_explicit_policy_is_refused_and_only_failed_plugins_restart | §19.2(7) escalation (lifecycle-state abuse); §5.6 (no auto-restart; the explicit restart is the only path) |
// | 5 | a_drain_deadline_abuse_cannot_force_a_replacement | §19.2(7) escalation (drain-deadline abuse); §5.6 drain-before-replace (fail-closed; the operator decides) |
// | 6 | a_forged_profile_cannot_escalate_the_ceiling_or_the_control_authority_mode | §19.2(7) escalation (profile-ceiling escalation via a forged profile); §5.8 mode lock; strict parse refusal |
//
// CLAIM BOUNDARY: these tests prove the KERNEL seams refuse every attack at
// the SDK boundary. They are NOT a security certification and claim none
// (AGENTS.md §3 rule 2). No cryptographic signature exists anywhere (the
// P7.14 digest-and-record honesty pattern: `SIGNING_STATEMENT`); the test
// verifier here is the labelled `UnsafeTestOnlyDigest` — the tamper path it
// exercises is structural, not a cryptographic-strength claim.

use flexcore_kernel::lifecycle::{
    DrainOutcome, DrainPort, DrainPolicy, FixedClock, GateOutcome, LifecycleAuditKind,
    LifecycleError, LifecycleState, MemoryAuditSink, PluginLifecycle,
};
use flexcore_kernel::manifest::{parse_manifest, SemVer};
use flexcore_kernel::profile::{parse_profile, validate_profile, ProfileError};
use flexcore_kernel::signing::{
    parse_provenance, provenance_gate, verify_artifact, ProvenanceRecord, ProvenanceVerdict,
    SigningError, UnsafeTestOnlyDigest, REFUSAL_DIGEST_MISMATCH, REFUSAL_MANIFEST_MISMATCH,
    REFUSAL_UNSIGNED,
};

// --- Shared fixtures -------------------------------------------------------------

const MANIFEST_ID: &str = "neryva.optimizer-wasm-lp";
/// The version the deployment verified (the honest baseline).
const VERIFIED_VERSION: SemVer = SemVer::new(2, 0, 0);
/// The version the attacker substitutes (the downgrade).
const SUBSTITUTED_VERSION: SemVer = SemVer::new(1, 9, 0);

const ARTIFACT_BYTES: &[u8] = b"plugin-artifact-bytes-for-the-p7-29-attack-chains";

fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

/// A provenance record over the intact artifact, as JSON, with the SBOM
/// reference controllable (the signer-side shape a deployment would emit;
/// `None` produces the record WITHOUT the sbom-reference field entirely).
fn provenance_json(sbom_reference: Option<&str>) -> String {
    let sbom_line = match sbom_reference {
        Some(reference) => format!(",\n  \"sbom-reference\": \"{reference}\""),
        None => String::new(),
    };
    format!(
        r#"{{
  "provenance-schema": "flexcore-plugin-provenance-v1",
  "artifact-id": "plugins/artifacts/neryva.optimizer-wasm-lp-2.0.0.wasm",
  "digest-algorithm": "fnv1a64-test",
  "digest": "{:016x}",
  "signer-key-reference": "key-store:release/plugin-signing-2026",
  "signed-at-millis": 1762348800000,
  "manifest-id": "{mid}",
  "manifest-version": "2.0.0",
  "builder-id": "ci-builder/flexcore-release-2026-11"{sbom_line}
}}"#,
        fnv1a64(ARTIFACT_BYTES),
        mid = MANIFEST_ID,
    )
}

fn valid_record() -> ProvenanceRecord {
    parse_provenance(&provenance_json(Some("sbom/neryva.optimizer-wasm-lp-2.0.0.cdx.json")))
        .expect("the fixture record must parse")
}

/// Walk the provenance gate with the given record/verifier; returns the
/// outcome and the audit sink (timestamps from the injected fixed clock).
fn run_gate(
    record: Option<&ProvenanceRecord>,
    artifact: &[u8],
    expected_version: SemVer,
) -> (GateOutcome, MemoryAuditSink) {
    let audit = MemoryAuditSink::new();
    let clock = FixedClock::new(1_000);
    let outcome = provenance_gate(
        record,
        artifact,
        MANIFEST_ID,
        expected_version,
        &UnsafeTestOnlyDigest,
        &clock,
        &audit,
    );
    (outcome, audit)
}

/// A Class A fixture manifest (mirrors the P7.15 seed fixture shape) with the
/// plugin version parameterized — the substitution attack swaps exactly this
/// field.
fn manifest_json(plugin_id: &str, version: &str) -> String {
    format!(
        r#"{{
  "manifest-schema": "flexcore-plugin-manifest-v1",
  "plugin-id": "{plugin_id}",
  "name": "P7.29 security fixture for {plugin_id}",
  "version": "{version}",
  "api-version": ">=0.5.0",
  "class": "builtin-rust",
  "entry-point": {{
    "registered-type": "{plugin_id}/default"
  }},
  "provider-of": [
    "optimization.solver"
  ],
  "consumes": [],
  "permissions": [],
  "configuration-schema": {{
    "type": "object",
    "additionalProperties": false,
    "properties": {{}},
    "required": []
  }},
  "secrets": [],
  "resources": {{
    "max-memory-mb": 64,
    "timeout-ms": 30000
  }},
  "metadata": {{
    "license": "Apache-2.0",
    "description": "Test fixture manifest (not a shipped plugin)."
  }}
}}"#
    )
}

/// The honest deployment profile pinning the verified version (the P7.15
/// shape; not a shipped profile).
fn profile_json(version_constraint: &str) -> String {
    format!(
        r#"{{
  "profile-schema": "flexcore-profile-v1",
  "profile-id": "p729-security-fixture",
  "name": "P7.29 security-scenario fixture profile",
  "version": "0.5.0",
  "description": "Fixture profile for the P7.29 supply-chain and escalation attack scenarios (not a shipped profile).",
  "control-authority": "simulation-shadow",
  "permission-ceiling": [
    "telemetry.read"
  ],
  "selections": [
    {{
      "manifest-id": "neryva.optimizer-wasm-lp",
      "version-constraint": "{version_constraint}"
    }}
  ]
}}"#
    )
}

/// Drive one plugin's lifecycle from registration to Ready (the P7.22
/// helper's shape, restated locally).
fn drive_to_ready(lifecycle: &mut PluginLifecycle<'_>) {
    lifecycle.verify(GateOutcome::Passed, "p729", "provenance gate passes").expect("verified");
    lifecycle.mark_compatible(GateOutcome::Passed, "p729", "compatible").expect("compatible");
    lifecycle.configure(GateOutcome::Passed, "p729", "configured").expect("configured");
    lifecycle.start(GateOutcome::Passed, "p729", "plan resolved").expect("started");
    lifecycle.mark_ready(GateOutcome::Passed, "p729", "health passes").expect("ready");
}

// --- #1: tampered artifact, reverification at the boundary --------------------------

#[test]
fn a_tampered_artifact_is_refused_at_reverification_and_never_advances() {
    // Baseline: the honest pair verifies and the lifecycle commits Verified.
    let record = valid_record();
    let (outcome, _gate_audit) = run_gate(Some(&record), ARTIFACT_BYTES, VERIFIED_VERSION);
    assert!(outcome.passed(), "the honest pair passes the gate");

    // THE ATTACK: the artifact bytes are swapped AFTER verification (the
    // supply-chain tamper) and the pair is re-presented at the next
    // verification boundary (re-install, re-publish, composition retry).
    let mut substituted = ARTIFACT_BYTES.to_vec();
    substituted[3] ^= 0x40;
    let (outcome, gate_audit) = run_gate(Some(&record), &substituted, VERIFIED_VERSION);
    let reason = match &outcome {
        GateOutcome::Failed { reason } => reason.as_str(),
        GateOutcome::Passed => panic!("a substituted artifact must not pass reverification"),
    };
    assert!(
        reason.starts_with(REFUSAL_DIGEST_MISMATCH),
        "named refusal expected: {reason}"
    );
    // The refusal is audited with the attempted Discovered -> Verified edge.
    let gate_records = gate_audit.records();
    assert_eq!(gate_records.len(), 1);
    assert_eq!(gate_records[0].kind, LifecycleAuditKind::Refused);
    assert_eq!(gate_records[0].from, Some(LifecycleState::Discovered));
    assert_eq!(gate_records[0].to, Some(LifecycleState::Verified));

    // CONTAINMENT: handed the failed gate outcome, the lifecycle refuses the
    // transition fail-closed and the plugin NEVER ADVANCES — the §5.6 default
    // for a refused first gate (P7.07 DEFAULT decision: no Discovered ->
    // Rejected edge exists, so the refused plugin stays Discovered, refused
    // and audited; it simply never runs).
    let lifecycle_audit = MemoryAuditSink::new();
    let clock = FixedClock::new(2_000);
    let mut lifecycle =
        PluginLifecycle::register(MANIFEST_ID, "2.0.0", &clock, &lifecycle_audit, DrainPolicy::default());
    let err = lifecycle
        .verify(outcome, "composition-root", "substituted artifact re-presented")
        .expect_err("the gate failed; the transition must be refused");
    assert_eq!(err.name(), "GATE_REFUSED");
    assert_eq!(lifecycle.state(), LifecycleState::Discovered);
    let records = lifecycle_audit.records();
    let last = records.last().expect("the refusal is audited");
    assert_eq!(last.kind, LifecycleAuditKind::Refused);
}

// --- #2: version-downgrade substitution ----------------------------------------------

#[test]
fn a_version_downgrade_substitution_is_refused_by_the_profile_constraint_and_the_provenance_binding() {
    // Baseline: the profile pins the verified version; the verified manifest
    // composes (the positive control proving the refusal below is the
    // DOWNGRADE, not the fixture).
    let verified_manifest =
        parse_manifest(&manifest_json(MANIFEST_ID, "2.0.0")).expect("fixture manifest parses");
    let profile = parse_profile(&profile_json(">=2.0.0")).expect("the honest profile parses");
    let plan = validate_profile(&profile, &[verified_manifest])
        .expect("the verified version satisfies the pin");
    assert_eq!(plan.registrations.len(), 1);

    // THE ATTACK, layer 1 (composition): the same plugin id substituted at a
    // LOWER version. The profile version-constraint refuses it by name.
    let downgraded =
        parse_manifest(&manifest_json(MANIFEST_ID, "1.9.0")).expect("the substituted manifest parses");
    let err = validate_profile(&profile, &[downgraded])
        .expect_err("a downgrade below the pinned constraint is refused");
    assert_eq!(err.name(), "PROFILE_MANIFEST_VERSION_UNSATISFIED");

    // THE ATTACK, layer 2 (the Verified gate): even past composition, the
    // provenance record BINDS the verified manifest version — the record
    // that legitimately proves 2.0.0 refuses to verify the substituted
    // 1.9.0 manifest (the record refuses the substitution before any digest
    // work: the binding check runs first).
    let record = valid_record();
    let refused = verify_artifact(
        &record,
        ARTIFACT_BYTES,
        MANIFEST_ID,
        SUBSTITUTED_VERSION,
        &UnsafeTestOnlyDigest,
    );
    match refused {
        ProvenanceVerdict::Refused { reason } => {
            assert!(
                reason.starts_with(REFUSAL_MANIFEST_MISMATCH),
                "named refusal expected: {reason}"
            );
        }
        ProvenanceVerdict::Accepted => {
            panic!("a record bound to 2.0.0 must not verify a 1.9.0 manifest")
        }
    }
}

// --- #3: unprovenanced plugin (no SBOM reference / no record) ------------------------

#[test]
fn an_unprovenanced_plugin_is_refused_before_any_compatibility_check() {
    // Attack shape 1: a record that is otherwise complete but carries NO
    // sbom-reference field — the closed key set refuses it at parse time
    // (§9.3: an artifact without an SBOM reference is not provenanced).
    let missing_sbom = provenance_json(None);
    let err = parse_provenance(&missing_sbom).expect_err("a record without an SBOM reference is refused");
    assert_eq!(err.name(), "PROVENANCE_MISSING_FIELD");
    assert!(
        matches!(&err, SigningError::RecordMissingField { field, .. } if field == "sbom-reference"),
        "the missing field is exactly sbom-reference: {err}"
    );

    // Attack shape 2: an EMPTY sbom-reference — the named SBOM refusal.
    let empty_sbom = provenance_json(Some(""));
    let err = parse_provenance(&empty_sbom).expect_err("an empty SBOM reference is refused");
    assert_eq!(err.name(), "PROVENANCE_SBOM_REFERENCE_INVALID");

    // Attack shape 3: no record at all. The gate refuses
    // `UNSIGNED_ARTIFACT` BEFORE any compatibility check (the P7.14 rule),
    // the refusal is audited with the attempted edge, and the lifecycle
    // refuses the transition — the plugin never advances.
    let (outcome, gate_audit) = run_gate(None, ARTIFACT_BYTES, VERIFIED_VERSION);
    let reason = match &outcome {
        GateOutcome::Failed { reason } => reason.as_str(),
        GateOutcome::Passed => panic!("an unprovenanced artifact must not pass"),
    };
    assert!(reason.starts_with(REFUSAL_UNSIGNED), "named refusal expected: {reason}");
    let gate_records = gate_audit.records();
    assert_eq!(gate_records.len(), 1);
    assert_eq!(gate_records[0].kind, LifecycleAuditKind::Refused);

    let lifecycle_audit = MemoryAuditSink::new();
    let clock = FixedClock::new(3_000);
    let mut lifecycle =
        PluginLifecycle::register(MANIFEST_ID, "2.0.0", &clock, &lifecycle_audit, DrainPolicy::default());
    let err = lifecycle
        .verify(outcome, "composition-root", "unprovenanced artifact")
        .expect_err("the gate failed; the transition must be refused");
    assert_eq!(err.name(), "GATE_REFUSED");
    assert_eq!(lifecycle.state(), LifecycleState::Discovered, "the plugin never advances");
}

// --- #4: restart without explicit policy (lifecycle-state abuse) ---------------------

#[test]
fn a_restart_without_explicit_policy_is_refused_and_only_failed_plugins_restart() {
    // Abuse 1: an Active (Ready) plugin "restarted" to shed its drain
    // obligation — the explicit restart is only legal from Failed; from any
    // other state it is the named illegal-transition refusal. (The call
    // consumes the lifecycle on either outcome, so this is that lifecycle's
    // final act by construction.)
    let clock = FixedClock::new(1_000);
    let audit = MemoryAuditSink::new();
    let mut lifecycle =
        PluginLifecycle::register(MANIFEST_ID, "2.0.0", &clock, &audit, DrainPolicy::default());
    drive_to_ready(&mut lifecycle);
    let err = lifecycle
        .restart_explicitly("operator", "abuse: restart to shed the drain obligation")
        .expect_err("a restart from Ready is refused (no auto-restart; the operator decides)");
    assert_eq!(err.name(), "ILLEGAL_TRANSITION");

    // Abuse 2: after a crash, the failed lifecycle CANNOT be walked forward
    // through the gates (there is no auto-restart), it STAYS Failed however
    // long the clock runs, and the failed-from state is retained as drill
    // evidence (the P7.30 failure-mode drill's input).
    let clock = FixedClock::new(10_000);
    let audit = MemoryAuditSink::new();
    let mut lifecycle =
        PluginLifecycle::register(MANIFEST_ID, "2.0.0", &clock, &audit, DrainPolicy::default());
    lifecycle.crash("host", "component crashed mid-schedule").expect("crash is representable");
    assert_eq!(lifecycle.state(), LifecycleState::Failed);
    assert_eq!(lifecycle.failed_from(), Some(LifecycleState::Discovered));
    let err = lifecycle
        .mark_compatible(GateOutcome::Passed, "attacker", "walk forward out of Failed")
        .expect_err("a gated forward step out of Failed is illegal (no auto-restart)");
    assert_eq!(err.name(), "ILLEGAL_TRANSITION");
    clock.advance_by(86_400_000);
    assert_eq!(lifecycle.state(), LifecycleState::Failed, "time alone never restarts a plugin");

    // The LEGITIMATE path: the explicit, audited restart from Failed — the
    // ONLY restart in v0.5.0 — yields a fresh lifecycle at Discovered.
    let fresh = lifecycle
        .restart_explicitly("operator", "explicit operator decision after post-mortem")
        .expect("the explicit restart is the only path out of Failed");
    assert_eq!(fresh.state(), LifecycleState::Discovered);
    let restart_record = audit
        .records()
        .iter()
        .find(|r| r.kind == LifecycleAuditKind::ExplicitRestart)
        .expect("the restart is audited as EXPLICIT_RESTART");
    assert_eq!(restart_record.from, Some(LifecycleState::Failed));
    assert_eq!(restart_record.to, Some(LifecycleState::Discovered));
    assert_eq!(restart_record.actor, "operator");
}

// --- #5: drain-deadline abuse to force a replacement ----------------------------------

/// A drain port that never quiesces (the stuck plugin) — the abuse tool: an
/// attacker (or a buggy operator script) forces the deadline to expire hoping
/// the expiry makes the incumbent removable.
struct StickyDrain {
    outstanding: usize,
}

impl DrainPort for StickyDrain {
    fn outstanding_work(&self) -> usize {
        self.outstanding
    }
    fn drain(&mut self, _deadline_millis: u64) -> DrainOutcome {
        DrainOutcome::DeadlineExceeded { outstanding: self.outstanding }
    }
}

/// A cooperative drain port (quiesces immediately; cancels its work).
struct CooperativeDrain {
    outstanding: usize,
}

impl DrainPort for CooperativeDrain {
    fn outstanding_work(&self) -> usize {
        self.outstanding
    }
    fn drain(&mut self, _deadline_millis: u64) -> DrainOutcome {
        let cancelled = self.outstanding;
        self.outstanding = 0;
        DrainOutcome::Quiesced { cancelled }
    }
}

#[test]
fn a_drain_deadline_abuse_cannot_force_a_replacement() {
    let clock = FixedClock::new(20_000);
    let audit = MemoryAuditSink::new();
    // Ports declared before the lifecycle (borrow/drop-order safety).
    let mut sticky = StickyDrain { outstanding: 2 };
    let mut cooperative = CooperativeDrain { outstanding: 2 };
    let mut lifecycle = PluginLifecycle::register(
        MANIFEST_ID,
        "2.0.0",
        &clock,
        &audit,
        DrainPolicy { deadline_ms: 1_000 },
    );
    drive_to_ready(&mut lifecycle);

    // THE ABUSE: begin the drain, let the deadline expire with work
    // outstanding. The stop is REFUSED (fail-closed) and the incumbent is
    // RESTORED to Ready (active) — the expiry makes the plugin MORE
    // protected, not less.
    lifecycle
        .begin_drain(&mut sticky, "operator", "abuse: force the deadline to force the replacement")
        .expect("the drain begins from Ready");
    let err = lifecycle
        .complete_drain("operator", "complete the (stuck) drain")
        .expect_err("the deadline expiry refuses the stop");
    assert_eq!(err.name(), "DRAIN_DEADLINE_EXCEEDED");
    assert_eq!(lifecycle.state(), LifecycleState::Ready, "the incumbent is restored to Ready (active)");
    let records = audit.records();
    let refused = &records[records.len() - 2];
    let restored = records.last().expect("the restoration is audited");
    assert_eq!(refused.kind, LifecycleAuditKind::Refused);
    assert_eq!(refused.from, Some(LifecycleState::Draining));
    assert_eq!(refused.to, Some(LifecycleState::Stopped));
    assert_eq!(restored.kind, LifecycleAuditKind::Transition);
    assert_eq!(restored.from, Some(LifecycleState::Draining));
    assert_eq!(restored.to, Some(LifecycleState::Ready));

    // The replacement is STILL blocked: the restored incumbent is not
    // drained, and the refusal is audited (the P7.08 registry's
    // `swap_provider` calls exactly this gate — cited, kernel-internal here).
    let err = lifecycle
        .require_fully_drained("attacker", "abuse: replace the restored incumbent")
        .expect_err("a restored (undrained) incumbent blocks any replacement");
    assert_eq!(err.name(), "REPLACEMENT_BLOCKED_UNDRAINED");
    let last = audit.records().last().expect("the replacement refusal is audited");
    assert_eq!(last.kind, LifecycleAuditKind::Refused);

    // The ONLY way through is a genuinely quiesced drain: the operator (or
    // the plugin) finishes the work, the drain completes, and the replacement
    // gate opens — never the deadline abuse.
    lifecycle
        .begin_drain(&mut cooperative, "operator", "the operator retries after the plugin quiesced")
        .expect("a fresh drain may begin from the restored Ready state");
    let outcome = lifecycle
        .complete_drain("operator", "the cooperative drain completes")
        .expect("the cooperative drain quiesces");
    assert_eq!(outcome, DrainOutcome::Quiesced { cancelled: 2 });
    assert_eq!(lifecycle.state(), LifecycleState::Stopped);
    lifecycle
        .require_fully_drained("operator", "the legitimate replacement after quiescence")
        .expect("a fully drained incumbent may be replaced");
}

// --- #6: forged-profile ceiling escalation ---------------------------------------------

#[test]
fn a_forged_profile_cannot_escalate_the_ceiling_or_the_control_authority_mode() {
    // The honest fixture (positive control): parses, locks the posture, and
    // composes against the verified manifest.
    let honest = parse_profile(&profile_json(">=2.0.0")).expect("the honest profile parses");
    assert_eq!(honest.control_authority().name(), "simulation-shadow");
    let verified =
        parse_manifest(&manifest_json(MANIFEST_ID, "2.0.0")).expect("fixture manifest parses");
    validate_profile(&honest, &[verified]).expect("the honest profile composes");

    // FORGERY 1: the ceiling edited to admit a never-granted id (the
    // escalation target set — no profile can ceiling what no manifest may
    // request). Strict parse refusal, never a warning.
    let forged = profile_json(">=2.0.0").replace("\"telemetry.read\"", "\"dispatch.authorize\"");
    let err = parse_profile(&forged).expect_err("a forged ceiling entry is refused at parse");
    assert_eq!(err.name(), "PROFILE_PERMISSION_CEILING_NEVER_GRANTED");

    // FORGERY 2: the control-authority mode edited to a higher §21.4 mode —
    // the mode lock refuses any value other than `simulation-shadow`; a
    // profile (like a plugin) can never select or raise the mode.
    let forged = profile_json(">=2.0.0")
        .replace("\"control-authority\": \"simulation-shadow\"", "\"control-authority\": \"operator-approved\"");
    let err = parse_profile(&forged).expect_err("a forged control-authority mode is refused");
    assert_eq!(err.name(), "CONTROL_AUTHORITY_MODE_FORBIDDEN");

    // FORGERY 3: a smuggled control knob (an unknown top-level field) —
    // unknown fields are rejected, never warned about.
    let forged =
        profile_json(">=2.0.0").replace("\"profile-id\"", "\"enable-dispatch\": true,\n  \"profile-id\"");
    let err = parse_profile(&forged).expect_err("a smuggled knob is an unknown-field rejection");
    assert_eq!(err.name(), "PROFILE_UNKNOWN_FIELD");

    // FORGERY 4: a ceiling entry outside the §5.7 vocabulary entirely —
    // equally refused (there is no id to escalate WITH that the vocabulary
    // does not name).
    let forged = profile_json(">=2.0.0").replace("\"telemetry.read\"", "\"kernel.command-authorizer\"");
    let err = parse_profile(&forged).expect_err("a ceiling entry outside the vocabulary is refused");
    assert_eq!(err.name(), "PROFILE_PERMISSION_CEILING_UNKNOWN");
}

// --- Named-error distinctness across the P7.29 refusal set ---------------------------

#[test]
fn every_refusal_name_asserted_by_this_suite_is_distinct() {
    // The named refusals this suite's attack chains pin, collected and
    // checked for pairwise distinctness (the P6.19/P7.05 table discipline):
    // two attacks that refuse with the SAME name would be indistinguishable
    // to an auditor; each class of attempt must refuse with its own name.
    let mut names = vec![
        "GATE_REFUSED",
        "ILLEGAL_TRANSITION",
        "DRAIN_DEADLINE_EXCEEDED",
        "REPLACEMENT_BLOCKED_UNDRAINED",
        "PROVENANCE_MISSING_FIELD",
        "PROVENANCE_SBOM_REFERENCE_INVALID",
        "PROFILE_MANIFEST_VERSION_UNSATISFIED",
        "PROFILE_PERMISSION_CEILING_NEVER_GRANTED",
        "PROFILE_PERMISSION_CEILING_UNKNOWN",
        "PROFILE_UNKNOWN_FIELD",
        "CONTROL_AUTHORITY_MODE_FORBIDDEN",
    ];
    names.sort();
    let len = names.len();
    names.dedup();
    assert_eq!(names.len(), len, "every asserted refusal name must be pairwise distinct");

    // And the lifecycle error names back the asserted strings (the names are
    // read from the code, not just written down here).
    assert_eq!(LifecycleError::GateRefused {
        from: LifecycleState::Discovered,
        to: LifecycleState::Verified,
        gate: "PROVENANCE",
        reason: "fixture".to_string(),
    }
    .name(), "GATE_REFUSED");
    assert_eq!(
        LifecycleError::DrainDeadlineExceeded { outstanding: 1 }.name(),
        "DRAIN_DEADLINE_EXCEEDED"
    );
    assert_eq!(
        LifecycleError::ReplacementBlockedUndrained {
            plugin_id: MANIFEST_ID.to_string(),
            state: LifecycleState::Ready,
        }
        .name(),
        "REPLACEMENT_BLOCKED_UNDRAINED"
    );
}
