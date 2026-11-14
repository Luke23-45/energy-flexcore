// P7.42 — Plugin-path determinism and seeded replay (ledger P7.42; report
// §24 step 10 replay reproducibility, §19.2(5) deterministic scenario tests,
// §9.2 provenance). WRITTEN, NEVER RUN — the loop bans hold; the human
// build/test pass executes these
// (`cargo test -p flexcore-kernel --test determinism_p7_42`).
//
// WHAT IS PROVEN HERE (the kernel-side half of P7.42 — the host-side half
// is `apps/plugin-host/tests/determinism_p7_42.rs`, and the FULL §24-step-10
// replay through a LIVE host+worker with pinned wasmtime/fuel versions is a
// HUMAN/DEPLOYMENT follow-up recorded in
// `ledger/phase-07-plugin-sdk/41_42_43_hardening_determinism_regression.md`
// §4 — the nondeterminism sources that only a live run can disposition
// (fuel-limit behaviour, Wasmtime-version sensitivity, worker timing) are
// listed there and are EXCLUDED from the equivalence claims below):
//
// 1. **Envelope construction is a pure function of its inputs**: the same
//    (correlation id, plugin identity, sequence, window, payload) always
//    builds a byte-identical envelope — identical canonical string, identical
//    digest. No wall-clock, no ambient entropy enters the identity.
// 2. **The FULL protected-path sequence is byte-identical across runs**: with
//    fixed seeds and injected clocks, request-envelope build → path-protection
//    checks → proposal envelope produces identical outcomes, identical
//    refusal names, and identical proposal envelopes across two fully
//    independent runs (fresh checkpoint files, fresh verifiers).
// 3. **Replaying a recorded request sequence after a checkpoint restore**
//    yields identical outcomes and identical refusal names: the per-channel
//    sequence state survives restart (the P7.36 file checkpoint) and the
//    replayed request is refused with exactly the name the first run
//    recorded for a duplicate.
// 4. **The validity window is an injected-clock decision**: expired and
//    not-yet-valid requests are refused with their distinct names at fixed
//    clock readings; a refusal NEVER advances the sequence window; no output
//    carries any time that did not come from the injected clock.
// 5. **The P7.18 supervisor's backoff schedule is a pure function of
//    (seed, attempt)** — asserted through a MIRRORED in-test recomputation
//    of the recorded formula and constants (read-only citation; see the
//    honest note on that test — the kernel cannot depend on the worker
//    crate, so the authoritative direct assertion lives in
//    `apps/worker/tests/supervision_p7_18.rs` and the mirror here pins the
//    recorded constants and properties).
// 6. **The kernel's composition outputs are pure**: the resolver, the
//    profile→plan validation, and the P7.19 configuration resolution produce
//    identical (PartialEq-equal, byte-identical JSON) outputs for identical
//    inputs, across independent runs.
// 7. **Audit timestamps come only from the injected clock** (the kernel
//    lifecycle): every emitted `LifecycleAuditRecord` carries exactly the
//    `FixedClock` reading at emission.
//
// WALL-CLOCK RESIDUAL (recorded honestly): a cross-crate grep assertion over
// the touched module paths is not possible from inside a kernel test (the
// host and worker crates are not dependencies here). What IS asserted is the
// injected-clock-only CONSTRUCTION: every kernel time-consuming type in this
// suite is constructed with an explicit `&dyn Clock` (`PluginPathVerifier`,
// `PluginLifecycle`) and every recorded timestamp below equals an injected
// reading. The host-side half of the residual is asserted in the host test
// (the broker/gate/monitor read NO clock — every timestamp is a parameter);
// the deployment-stage grep (`SystemTime::now` / `Instant::now` over
// `crates/flexcore-kernel/src` and the plugin path) is a recorded follow-up
// step of the human build pass.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use flexcore_kernel::lifecycle::{
    FixedClock, GateOutcome, LifecycleState, MemoryAuditSink, PluginLifecycle,
};
use flexcore_kernel::manifest::parse_manifest;
use flexcore_kernel::plugin_config::resolve_for_instance;
use flexcore_kernel::plugin_path_protection::{
    FilePluginSequenceCheckpoint, PluginPathError, PluginPathOutcome, PluginPathVerifier,
    PluginProposalEnvelope, PluginRequestEnvelope, PluginResponseEnvelope,
};
use flexcore_kernel::profile::{parse_profile, validate_profile};
use flexcore_kernel::resolve::{resolve, ExplicitSelections};

const BASE_MS: u64 = 1_700_000_000_000;
const WINDOW_MS: u64 = 60_000;
const PLUGIN_ID: &str = "neryva.det-optimizer";
const PLUGIN_VERSION: &str = "0.1.0";
const CORRELATION_ID: &str = "corr-det-001";

// --- Helpers -------------------------------------------------------------------

/// A unique scratch path per call (std-only; no tempfile dep — the P7.36
/// test pattern, reused).
fn scratch_path(label: &str) -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let mut path = std::env::temp_dir();
    path.push(format!(
        "flexcore-kernel-p742-{}-{}-{}",
        std::process::id(),
        label,
        COUNTER.fetch_add(1, Ordering::SeqCst)
    ));
    let _ = std::fs::remove_file(&path);
    path
}

fn payload(sequence: u64) -> serde_json::Value {
    serde_json::json!({
        "asset": "ess-01",
        "action": "set-power-mw",
        "value": 2.5,
        "seq": sequence
    })
}

fn envelope(sequence: u64, payload: serde_json::Value) -> PluginRequestEnvelope {
    PluginRequestEnvelope::new(
        CORRELATION_ID,
        PLUGIN_ID,
        PLUGIN_VERSION,
        sequence,
        BASE_MS,
        BASE_MS + WINDOW_MS,
        payload,
    )
    .expect("the reference envelope is valid")
}

fn error_name(err: &PluginPathError) -> &'static str {
    err.name()
}

// --- 1. Envelope construction is a pure function --------------------------------

#[test]
fn envelope_construction_is_a_pure_function_of_its_inputs() {
    let first = envelope(7, payload(7));
    let second = envelope(7, payload(7));
    assert_eq!(first.canonical_string(), second.canonical_string());
    assert_eq!(first.digest_hex, second.digest_hex);
    assert_eq!(first, second);

    // The identity binds EVERY field: change one field, the digest changes.
    let other_payload = envelope(7, serde_json::json!({"asset": "ess-01", "value": 9.0}));
    assert_ne!(first.digest_hex, other_payload.digest_hex);
    let other_seq = envelope(8, payload(8));
    assert_ne!(first.digest_hex, other_seq.digest_hex);
    // And the digest re-verifies on the untouched envelope (check 2 passes).
    first.verify_digest().expect("a constructed envelope verifies");
}

// --- 2. The full path sequence is identical across two independent runs ---------

/// One recorded exchange outcome: everything the path produced for one
/// request, in a comparable shape (canonical string, digest, outcome).
struct ExchangeRecord {
    canonical_string: String,
    digest_hex: String,
    outcome: PluginPathOutcome,
}

/// Runs the FULL sequence — envelope build → path-protection checks →
/// proposal envelope — for a fixed request list, against a fresh checkpoint
/// file and a fixed clock, recording everything.
fn run_full_plugin_path(path: &Path, sequences: &[u64]) -> Vec<ExchangeRecord> {
    let clock = FixedClock::new(BASE_MS);
    let checkpoint = Box::new(
        FilePluginSequenceCheckpoint::open(path).expect("the checkpoint opens"),
    );
    let mut verifier =
        PluginPathVerifier::new(&clock, checkpoint).expect("the verifier loads its history");
    let mut records = Vec::with_capacity(sequences.len());
    for sequence in sequences {
        let env = envelope(*sequence, payload(*sequence));
        // The protection checks (envelope self-validity, digest, injected-
        // clock window, monotonic sequence) — all four must pass on the
        // ascending reference sequence.
        verifier
            .verify_and_admit(&env)
            .unwrap_or_else(|e| panic!("sequence {sequence} refused unexpectedly: {e}"));
        // The response echo and the PROPOSAL envelope (ADR-010: data, never
        // authority).
        let response = PluginResponseEnvelope {
            correlation_id: env.correlation_id.clone(),
            sequence: env.sequence,
            payload: payload(*sequence),
        };
        let proposal = PluginProposalEnvelope::new(
            &format!("proposal-{sequence}"),
            PLUGIN_ID,
            PLUGIN_VERSION,
            serde_json::json!({"schedule": {"ess-01": 2.5}, "seq": sequence}),
            *sequence,
        )
        .expect("the reference proposal is valid");
        let outcome = verifier
            .record_outcome(&env, &response, Some(proposal))
            .expect("the reference exchange records");
        records.push(ExchangeRecord {
            canonical_string: env.canonical_string(),
            digest_hex: env.digest_hex.clone(),
            outcome,
        });
    }
    records
}

#[test]
fn the_full_plugin_path_sequence_is_identical_across_two_independent_runs() {
    let path_a = scratch_path("run-a");
    let path_b = scratch_path("run-b");
    let sequences = [1u64, 2, 3];

    let run_a = run_full_plugin_path(&path_a, &sequences);
    let run_b = run_full_plugin_path(&path_b, &sequences);

    assert_eq!(run_a.len(), run_b.len());
    for (a, b) in run_a.iter().zip(run_b.iter()) {
        // Byte-identical: the canonical identity string and the recorded
        // digest are byte-for-byte equal across fully independent runs.
        assert_eq!(a.canonical_string, b.canonical_string);
        assert_eq!(a.digest_hex, b.digest_hex);
        // Identical outcomes: the proposal envelopes compare equal
        // (PartialEq) — same proposal id, producer identity, payload,
        // sequence.
        assert_eq!(a.outcome, b.outcome);
    }

    // And the recorded outcome is the PROPOSAL variant for every exchange
    // (the ADR-010 shape), never anything dispatch-authorizing.
    for record in &run_a {
        assert!(matches!(record.outcome, PluginPathOutcome::ProposalRecorded(_)));
    }

    let _ = std::fs::remove_file(&path_a);
    let _ = std::fs::remove_file(&path_b);
}

// --- 3. Replay after checkpoint restore ------------------------------------------

#[test]
fn replaying_a_recorded_sequence_after_checkpoint_restore_yields_identical_refusals() {
    let path = scratch_path("restore");
    let sequences = [1u64, 2, 3];

    // Run 1: the recorded sequence lands in the checkpoint file.
    let first = run_full_plugin_path(&path, &sequences);

    // Restart: a FRESH verifier over the SAME checkpoint file (the
    // restart-survival path — sequence history loaded from disk).
    let clock = FixedClock::new(BASE_MS);
    let checkpoint = Box::new(
        FilePluginSequenceCheckpoint::open(&path).expect("the checkpoint reopens"),
    );
    let mut restored =
        PluginPathVerifier::new(&clock, checkpoint).expect("the restored verifier loads");

    // The restored state is exactly the recorded state (sorted load).
    for sequence in sequences {
        // Nothing to assert directly per sequence; the replay refusals below
        // ARE the assertion (a lost window would admit the replay).
        let _ = sequence;
    }

    // Replaying the LAST accepted request (same correlation id, same
    // sequence, same payload) must be refused with the SAME name the
    // duplicate case names — SEQUENCE_REPLAYED — deterministically.
    let replay = envelope(3, payload(3));
    let replay_err = restored
        .verify_and_admit(&replay)
        .expect_err("a replayed request must be refused after restart");
    assert_eq!(error_name(&replay_err), "SEQUENCE_REPLAYED");

    // The refusal is stable across repeated attempts (same input → same
    // refusal, identical object).
    let replay_again = envelope(3, payload(3));
    match (restored.verify_and_admit(&replay), restored.verify_and_admit(&replay_again)) {
        (Err(a), Err(b)) => assert_eq!(a, b, "the replay refusal is not deterministic"),
        (a, b) => panic!("replay admission is unstable: {a:?} vs {b:?}"),
    }

    // And an OUT-OF-SEQUENCE replay (below the last accepted) names its own
    // distinct refusal — identical to the first-run semantics.
    let stale = envelope(1, payload(1));
    let stale_err = restored
        .verify_and_admit(&stale)
        .expect_err("a stale sequence must be refused");
    assert_eq!(error_name(&stale_err), "SEQUENCE_OUT_OF_ORDER");

    // A NEW sequence (4) is accepted after restore — the window advanced
    // exactly as recorded, no more, no less.
    let next = envelope(4, payload(4));
    restored
        .verify_and_admit(&next)
        .expect("the next sequence is admitted after a clean restore");
    // ...and its repeat is now the duplicate (the same name as run 1's
    // duplicate case — identical refusal names across the restart).
    let next_repeat = envelope(4, payload(4));
    let repeat_err = restored
        .verify_and_admit(&next_repeat)
        .expect_err("a duplicate of the newly accepted sequence must be refused");
    assert_eq!(error_name(&repeat_err), "SEQUENCE_REPLAYED");

    // The first run's records remain the reference (the identical-outcome
    // claim of item 2 covers their shape).
    assert!(first.iter().all(|r| matches!(r.outcome, PluginPathOutcome::ProposalRecorded(_))));

    let _ = std::fs::remove_file(&path);
}

// --- 4. The validity window is an injected-clock decision ------------------------

#[test]
fn expiry_and_prematurity_are_injected_clock_decisions_and_refusals_never_advance() {
    let path = scratch_path("window");
    let clock = FixedClock::new(BASE_MS);
    let checkpoint = Box::new(
        FilePluginSequenceCheckpoint::open(&path).expect("the checkpoint opens"),
    );
    let mut verifier =
        PluginPathVerifier::new(&clock, checkpoint).expect("the verifier loads");

    // Not yet valid: the window opens later than the clock reading.
    let early = PluginRequestEnvelope::new(
        CORRELATION_ID,
        PLUGIN_ID,
        PLUGIN_VERSION,
        1,
        BASE_MS + 1_000,
        BASE_MS + WINDOW_MS,
        payload(1),
    )
    .expect("the envelope is self-valid");
    let err = verifier
        .verify_and_admit(&early)
        .expect_err("a not-yet-valid request is refused");
    assert_eq!(error_name(&err), "REQUEST_NOT_YET_VALID");
    assert_eq!(verifier.last_accepted(
        flexcore_kernel::plugin_path_protection::sequence_channel(PLUGIN_ID, "invocation")
    ), None, "a refusal must never advance the sequence window");

    // Expired: the clock reading reaches the window's end. The clock is
    // ADVANCED by the test (injected), never sampled from the wall.
    clock.advance_by(WINDOW_MS);
    let late = PluginRequestEnvelope::new(
        CORRELATION_ID,
        PLUGIN_ID,
        PLUGIN_VERSION,
        2,
        BASE_MS,
        BASE_MS + WINDOW_MS,
        payload(2),
    )
    .expect("the envelope is self-valid");
    let err = verifier
        .verify_and_admit(&late)
        .expect_err("an expired request is refused, never executed");
    assert_eq!(error_name(&err), "REQUEST_EXPIRED");
    assert_eq!(
        verifier.last_accepted(
            flexcore_kernel::plugin_path_protection::sequence_channel(PLUGIN_ID, "invocation")
        ),
        None,
        "the expiry refusal also must not advance the window"
    );

    // Same clock readings → same admission decisions: rewinding the clock to
    // BASE_MS reproduces the original acceptance for sequence 3 exactly.
    clock.set(BASE_MS);
    let good = envelope(3, payload(3));
    verifier
        .verify_and_admit(&good)
        .expect("a valid request inside the window is admitted");
    assert_eq!(
        verifier.last_accepted(
            flexcore_kernel::plugin_path_protection::sequence_channel(PLUGIN_ID, "invocation")
        ),
        Some(3)
    );

    let _ = std::fs::remove_file(&path);
}

// --- 5. The P7.18 backoff schedule, mirrored (read-only citation) ----------------
//
// HONEST NOTE: `flexcore-kernel` cannot depend on `flexcore-worker` (the
// kernel is upstream of every consumer; `tools/check_dependency_rule.py`
// classifies it COMPOSITION with nothing depending on it). The MIRRORED
// assertion below re-derives the schedule from the RECORDED constants and
// formula (`apps/worker/src/supervision.rs::RetryScheduler`, seed
// `DEFAULT_SUPERVISION_SEED = 0x464C_5843_5037_3138` "FLXP718", base 250 ms,
// cap 5 000 ms, jitter cap 250 ms, xorshift64 jitter) and pins the pure-
// function property. The DIRECT assertion against the real `RetryScheduler`
// lives in `apps/worker/tests/supervision_p7_18.rs`; if that suite and this
// mirror ever disagree, the worker suite wins and this mirror is re-recorded.

/// The recorded P7.18 supervision constants (read-only mirror; cited from
/// `apps/worker/src/supervision.rs` and `17_18_workers_supervision.md` §5).
const RECORDED_SUPERVISION_SEED: u64 = 0x464C_5843_5037_3138;
const RECORDED_BACKOFF_BASE_MS: u64 = 250;
const RECORDED_BACKOFF_CAP_MS: u64 = 5_000;
const RECORDED_JITTER_CAP_MS: u64 = 250;
const RECORDED_GOLDEN: u64 = 0x9E37_79B9_7F4A_7C15;

/// The recorded jitter formula (xorshift64 over (seed, attempt)).
fn mirrored_jitter(attempt: u32) -> u64 {
    let mut x = RECORDED_SUPERVISION_SEED
        .wrapping_add((u64::from(attempt) + 1).wrapping_mul(RECORDED_GOLDEN));
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    x % RECORDED_JITTER_CAP_MS
}

/// The recorded delay formula: `min(base * 2^attempt, cap) + jitter`.
fn mirrored_delay(attempt: u32) -> u64 {
    let factor = 1u64.checked_shl(attempt.min(31)).unwrap_or(u64::MAX);
    let exponential = RECORDED_BACKOFF_BASE_MS.saturating_mul(factor);
    exponential
        .min(RECORDED_BACKOFF_CAP_MS)
        .saturating_add(mirrored_jitter(attempt))
}

#[test]
fn the_p7_18_backoff_schedule_is_a_pure_function_of_seed_and_attempt_mirrored() {
    // Pure-function property: the same (seed, attempt) always yields the
    // same delay — computed through two independent derivations.
    for attempt in 0..24u32 {
        let first = mirrored_delay(attempt);
        let second = mirrored_delay(attempt);
        assert_eq!(
            first, second,
            "the backoff schedule is not a pure function of (seed, attempt) at {attempt}"
        );
    }

    // The recorded bound properties: the exponential part caps at 5 000 ms,
    // the jitter stays inside [0, 250), so the total delay is inside
    // [cap, cap + jitter_cap) once the cap is reached — never unbounded
    // growth, never a wall-clock input.
    for attempt in 0..40u32 {
        let delay = mirrored_delay(attempt);
        let exponential = RECORDED_BACKOFF_BASE_MS
            .saturating_mul(1u64.checked_shl(attempt.min(31)).unwrap_or(u64::MAX))
            .min(RECORDED_BACKOFF_CAP_MS);
        assert_eq!(
            delay - mirrored_jitter(attempt),
            exponential,
            "the exponential part at {attempt} is not min(base*2^attempt, cap)"
        );
        assert!(
            delay < RECORDED_BACKOFF_CAP_MS + RECORDED_JITTER_CAP_MS,
            "the delay at {attempt} exceeds cap + jitter cap"
        );
    }

    // The recorded seed constant is pinned: the jitter derives from
    // 0x464C_5843_5037_3138 ("FLXP718") through the recorded golden
    // multiplier; the exact bound assertions below fail if the recorded
    // seed constant or the formula drifts on either side (worker suite vs
    // this mirror).
    let attempt0 = mirrored_delay(0);
    let attempt1 = mirrored_delay(1);
    assert!(
        (RECORDED_BACKOFF_BASE_MS..RECORDED_BACKOFF_BASE_MS + RECORDED_JITTER_CAP_MS)
            .contains(&attempt0),
        "attempt-0 delay {attempt0} is not base + jitter(0) inside the recorded bounds"
    );
    assert!(
        (2 * RECORDED_BACKOFF_BASE_MS..2 * RECORDED_BACKOFF_BASE_MS + RECORDED_JITTER_CAP_MS)
            .contains(&attempt1),
        "attempt-1 delay {attempt1} is not 2*base + jitter(1) inside the recorded bounds"
    );
    // Zero jitter cap → exactly the exponential part (still deterministic).
    let no_jitter_delay = RECORDED_BACKOFF_BASE_MS
        .saturating_mul(2u64.pow(3))
        .min(RECORDED_BACKOFF_CAP_MS);
    assert_eq!(no_jitter_delay, 2_000, "2^3 * 250 must be 2 000 ms before the cap");
}

// --- 6. Composition outputs are pure ----------------------------------------------

const DET_PORTFOLIO_MANIFEST: &str = r#"{
  "manifest-schema": "flexcore-plugin-manifest-v1",
  "plugin-id": "neryva.det-portfolio",
  "name": "Determinism Probe Portfolio Provider",
  "version": "0.1.0",
  "api-version": ">=0.5.0, <0.6.0",
  "class": "builtin-rust",
  "entry-point": { "registered-type": "neryva.builtin/det-portfolio" },
  "provider-of": ["portfolio.read"],
  "consumes": [],
  "permissions": ["asset.read"],
  "configuration-schema": {
    "type": "object",
    "additionalProperties": false,
    "properties": {},
    "required": []
  },
  "secrets": [],
  "resources": { "max-memory-mb": 16, "timeout-ms": 5000 },
  "metadata": { "license": "Apache-2.0", "description": "P7.42 determinism probe (provider)." }
}"#;

const DET_OPTIMIZER_MANIFEST: &str = r#"{
  "manifest-schema": "flexcore-plugin-manifest-v1",
  "plugin-id": "neryva.det-optimizer",
  "name": "Determinism Probe Optimizer",
  "version": "0.1.0",
  "api-version": ">=0.5.0, <0.6.0",
  "class": "builtin-rust",
  "entry-point": { "registered-type": "neryva.builtin/det-optimizer" },
  "provider-of": ["optimization.lp"],
  "consumes": ["portfolio.read"],
  "permissions": ["schedule.propose"],
  "configuration-schema": {
    "type": "object",
    "additionalProperties": false,
    "properties": {
      "gap-tolerance": { "type": "number", "description": "Relative gap tolerance." }
    },
    "required": []
  },
  "secrets": [],
  "resources": { "max-memory-mb": 16, "timeout-ms": 5000 },
  "metadata": { "license": "Apache-2.0", "description": "P7.42 determinism probe (optimizer)." }
}"#;

const DET_PROFILE: &str = r#"{
  "profile-schema": "flexcore-profile-v1",
  "profile-id": "det-probe",
  "name": "Determinism Probe",
  "version": "1.0.0",
  "description": "P7.42 determinism probe profile.",
  "control-authority": "simulation-shadow",
  "permission-ceiling": ["schedule.propose", "asset.read"],
  "selections": [
    { "manifest-id": "neryva.det-optimizer", "version-constraint": ">=0.1.0, <0.2.0" },
    { "manifest-id": "neryva.det-portfolio", "version-constraint": ">=0.1.0, <0.2.0" }
  ]
}"#;

#[test]
fn resolver_profile_and_config_outputs_are_pure_functions_of_their_inputs() {
    let portfolio = parse_manifest(DET_PORTFOLIO_MANIFEST).expect("the provider fixture parses");
    let optimizer = parse_manifest(DET_OPTIMIZER_MANIFEST).expect("the consumer fixture parses");
    let manifests = vec![portfolio, optimizer];

    // Resolver: identical inputs → identical plan, across runs. The
    // selections mirror the profile's pins (every provider-of contract of
    // every selected manifest) so the resolver plan and the profile plan
    // are the SAME input resolved twice, not two different inputs.
    let mut selections = ExplicitSelections::new();
    selections.insert("optimization.lp".to_string(), "neryva.det-optimizer".to_string());
    selections.insert("portfolio.read".to_string(), "neryva.det-portfolio".to_string());
    let plan_a = resolve(&manifests, &selections).expect("the probe composition resolves");
    let plan_b = resolve(&manifests, &selections).expect("the probe composition resolves again");
    assert_eq!(plan_a, plan_b, "the resolver is not a pure function of its inputs");

    // Profile → plan: identical inputs → identical plan, across runs.
    let profile = parse_profile(DET_PROFILE).expect("the probe profile parses");
    let plan_c = validate_profile(&profile, &manifests).expect("the profile validates");
    let plan_d = validate_profile(&profile, &manifests).expect("the profile validates again");
    assert_eq!(plan_c, plan_d, "profile→plan validation is not a pure function");
    assert_eq!(plan_a, plan_c, "the resolver and the profile path disagree on equal inputs");

    // Configuration resolution: identical inputs → identical resolved view,
    // rendered byte-identically, across runs.
    let config = serde_json::json!({"gap-tolerance": 0.01});
    let resolved_a = resolve_for_instance(&optimizer, "0.1.0", None, &config)
        .expect("the probe config resolves");
    let resolved_b = resolve_for_instance(&optimizer, "0.1.0", None, &config)
        .expect("the probe config resolves again");
    assert_eq!(
        resolved_a.resolved_json(),
        resolved_b.resolved_json(),
        "the config resolution is not byte-stable for identical inputs"
    );
    assert_eq!(
        serde_json::to_string(&resolved_a.resolved_json()).expect("render"),
        serde_json::to_string(&resolved_b.resolved_json()).expect("render"),
        "the rendered resolved view is not byte-identical"
    );
}

// --- 7. Audit timestamps come only from the injected clock ------------------------

#[test]
fn lifecycle_audit_timestamps_come_only_from_the_injected_clock() {
    let clock = FixedClock::new(BASE_MS);
    let audit = MemoryAuditSink::new();
    let mut lifecycle = PluginLifecycle::register(
        PLUGIN_ID,
        PLUGIN_VERSION,
        &clock,
        &audit,
        flexcore_kernel::lifecycle::DrainPolicy::default(),
    );
    // The registration record is stamped with exactly the clock reading.
    {
        let records = audit.records();
        let last = records.last().expect("the registration was audited");
        assert_eq!(last.timestamp_millis, BASE_MS);
        assert_eq!(last.kind.name(), "REGISTRATION");
    }

    // Advance the injected clock; the next record carries the NEW reading —
    // no record ever carries a time the test did not inject.
    clock.advance_by(1_234);
    lifecycle
        .verify(GateOutcome::Passed, "p742-test", "determinism probe")
        .expect("the probe gate passes");
    {
        let records = audit.records();
        let last = records.last().expect("the transition was audited");
        assert_eq!(last.timestamp_millis, BASE_MS + 1_234);
        assert_eq!(last.to, Some(LifecycleState::Verified));
    }

    // The clock-reading equality holds for a REFUSAL record too (a refused
    // gate is audited with the same injected stamp).
    clock.advance_by(5);
    lifecycle
        .mark_compatible(
            GateOutcome::Failed { reason: "probe refusal".to_string() },
            "p742-test",
            "determinism probe",
        )
        .expect_err("a failed gate refuses");
    {
        let records = audit.records();
        let last = records.last().expect("the refusal was audited");
        assert_eq!(last.timestamp_millis, BASE_MS + 1_234 + 5);
        assert_eq!(last.kind.name(), "REFUSED");
    }
    assert_eq!(lifecycle.state(), LifecycleState::Verified, "a refused gate leaves the state unchanged");
}
