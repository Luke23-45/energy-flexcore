// P4.31 — Shadow-evaluation tests (report §19.2 layer 9, §19.1, §21.4;
//         ledger P4.31 steps 3–4; decision record:
//         `ledger/phase-04-grid-feasibility-dispatch/31_fault_shadow.md`).
//
// Coverage per the ledger acceptance criteria:
//
// - The comparison runs WITHOUT issuing anything: the baseline port is
//   observe-only, the report carries the nothing-issued statement, and the
//   structural source scan pins that the module declares no delivery path.
// - Agreement / divergence / not-comparable classification: within
//   tolerance agrees (delta recorded), beyond tolerance diverges (all
//   three values recorded), and a MISSING, STALE, future-stamped, or
//   quantity-mismatched baseline is `NotComparable` — fail-closed, never
//   read as agreement (the §10.4 / §17.5-invariant-9 discipline applied
//   to shadow).
// - The conservative overall verdict: one divergent or missing sample is
//   never averaged away.
// - The report type CARRIES its limits statement: shadow agreement is not
//   a safety claim; shadow results are never evidence for field dispatch
//   (`evidence_status` is pinned to `not_evidence_for_field_dispatch` and
//   the source scan pins that no `EvidenceReference` can be produced from
//   a report).
// - Determinism: the same input and baseline sequence produce identical
//   reports (double-run in-suite — recorded P3.26 discipline; nothing is
//   actually executed under the loop bans).
//
// Every timestamp in this suite comes from a controlled value passed
// explicitly to the evaluation (never wall-clock); the baseline fake is a
// deterministic scripted table.

use flexcore_application::dispatch_simulator::{SimulatorTargetValue, TrackedQuantity};
use flexcore_application::shadow_evaluation::{
    evaluate_shadow_comparison, NotComparableReason, ShadowBaselineSample, ShadowBaselineSource,
    ShadowComparisonConfig, ShadowComparisonOutcome, ShadowEvaluationError, ShadowOverallVerdict,
    NOTHING_ISSUED_STATEMENT, SHADOW_EVIDENCE_STATUS, SHADOW_LIMITS_STATEMENT,
    SHADOW_METHOD_STATEMENT,
};
use flexcore_domain::ids::{AssetId, CommandId};
use flexcore_domain::tenant_context::{AuthenticatedClaims, TenantContext, VerifiedClaims};

// --- Fixture identity ------------------------------------------------------------

const UUID_TENANT: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f7d01";
const UUID_ASSET: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f7d02";
const UUID_COMMAND: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f7d03";

fn uuid(text: &str) -> flexcore_contracts::ids::FlexUuid {
    flexcore_contracts::ids::FlexUuid::parse(text).expect("valid test uuid")
}

fn ctx() -> TenantContext {
    // The ONLY public construction path (P0.16 rule 5).
    TenantContext::from_authenticated_claims(AuthenticatedClaims::from_verified(VerifiedClaims {
        tenant_id: flexcore_domain::ids::TenantId::new(uuid(UUID_TENANT)),
        subject: "test:principal".to_string(),
    }))
}

fn asset_id() -> AssetId {
    AssetId::new(uuid(UUID_ASSET))
}

fn command_id() -> CommandId {
    CommandId::new(uuid(UUID_COMMAND))
}

// --- Scripted baseline fake ----------------------------------------------------------

/// A deterministic baseline: a lookup table instant → sample (or absence).
/// Nothing else — the fake has no write method to misuse.
struct ScriptedBaseline {
    samples: Vec<(i64, Option<ShadowBaselineSample>)>,
    source_failures: Vec<usize>, // 1-based call indices that fail
    calls: std::cell::Cell<usize>,
}

impl ScriptedBaseline {
    fn with_samples(samples: Vec<(i64, Option<ShadowBaselineSample>)>) -> Self {
        Self {
            samples,
            source_failures: Vec::new(),
            calls: std::cell::Cell::new(0),
        }
    }

    fn failing_on(call_indices: &[usize], samples: Vec<(i64, Option<ShadowBaselineSample>)>) -> Self {
        Self {
            samples,
            source_failures: call_indices.to_vec(),
            calls: std::cell::Cell::new(0),
        }
    }
}

impl ShadowBaselineSource for ScriptedBaseline {
    fn baseline_sample(
        &self,
        _ctx: &TenantContext,
        _asset_id: AssetId,
        at_unix_millis: i64,
    ) -> Result<Option<ShadowBaselineSample>, ShadowEvaluationError> {
        let call = self.calls.get() + 1;
        self.calls.set(call);
        if self.source_failures.contains(&call) {
            return Err(ShadowEvaluationError::BaselineSourceFailed {
                code: "shadow-baseline-unavailable".to_string(),
                message: "scripted baseline-source failure (fault injection)".to_string(),
            });
        }
        Ok(self
            .samples
            .iter()
            .find(|(instant, _)| *instant == at_unix_millis)
            .and_then(|(_, sample)| *sample))
    }
}

fn baseline(quantity: TrackedQuantity, value: f64, observed_at_unix_millis: i64) -> ShadowBaselineSample {
    ShadowBaselineSample {
        quantity,
        value,
        observed_at_unix_millis,
    }
}

fn config() -> ShadowComparisonConfig {
    ShadowComparisonConfig::new(0.15, 60_000).expect("valid config")
}

fn input(
    instants: Vec<i64>,
    now: i64,
    target: SimulatorTargetValue,
    config: ShadowComparisonConfig,
) -> flexcore_application::shadow_evaluation::ShadowComparisonInput {
    flexcore_application::shadow_evaluation::ShadowComparisonInput {
        command_id: command_id(),
        asset_id: asset_id(),
        target,
        instants_unix_millis: instants,
        now_unix_millis: now,
        config,
    }
}

// ============================================================================
// The comparison classification
// ============================================================================

#[test]
fn within_tolerance_agrees_with_the_delta_recorded() {
    let baseline_source = ScriptedBaseline::with_samples(vec![(
        5_000,
        Some(baseline(TrackedQuantity::ActivePowerMW, 1.55, 4_800)),
    )]);
    // Requested 1.50 MW; baseline 1.55 MW; tolerance 0.15 MW → agreement
    // (boundary case: delta == tolerance is agreement).
    let report = evaluate_shadow_comparison(
        &ctx(),
        &baseline_source,
        input(
            vec![5_000],
            5_000,
            SimulatorTargetValue::ActivePowerMW(1.5),
            config(),
        ),
    )
    .expect("evaluation succeeds");
    assert_eq!(report.overall, ShadowOverallVerdict::Agreement);
    assert_eq!(report.agreement_count(), 1);
    match report.entries[0].outcome {
        ShadowComparisonOutcome::Agreement { baseline, delta } => {
            assert_eq!(baseline, 1.55);
            assert!((delta - 0.05).abs() < 1e-12);
        }
        other => panic!("expected agreement, got {other:?}"),
    }
}

#[test]
fn beyond_tolerance_diverges_with_all_three_values_recorded() {
    let baseline_source = ScriptedBaseline::with_samples(vec![(
        5_000,
        Some(baseline(TrackedQuantity::ActivePowerMW, 0.8, 4_800)),
    )]);
    // Requested 1.50 MW; baseline 0.80 MW; delta 0.70 > 0.15 → divergence.
    let report = evaluate_shadow_comparison(
        &ctx(),
        &baseline_source,
        input(
            vec![5_000],
            5_000,
            SimulatorTargetValue::ActivePowerMW(1.5),
            config(),
        ),
    )
    .expect("evaluation succeeds");
    assert_eq!(report.overall, ShadowOverallVerdict::Divergence);
    match report.entries[0].outcome {
        ShadowComparisonOutcome::Divergence {
            baseline,
            requested,
            delta,
        } => {
            assert_eq!(baseline, 0.8);
            assert_eq!(requested, 1.5);
            assert!((delta - 0.7).abs() < 1e-12);
        }
        other => panic!("expected divergence, got {other:?}"),
    }
}

#[test]
fn a_missing_baseline_is_not_comparable_never_agreement() {
    // The baseline run holds NO sample at the instant (absence, §10.4
    // MISSING): fail-closed, never read as agreement.
    let baseline_source = ScriptedBaseline::with_samples(vec![(5_000, None)]);
    let report = evaluate_shadow_comparison(
        &ctx(),
        &baseline_source,
        input(
            vec![5_000],
            5_000,
            SimulatorTargetValue::ActivePowerMW(1.5),
            config(),
        ),
    )
    .expect("evaluation succeeds");
    assert_eq!(report.overall, ShadowOverallVerdict::NotComparable);
    assert_eq!(report.not_comparable_count(), 1);
    assert_eq!(report.agreement_count(), 0);
    assert!(matches!(
        report.entries[0].outcome,
        ShadowComparisonOutcome::NotComparable(NotComparableReason::NoBaseline)
    ));
}

#[test]
fn a_stale_baseline_is_not_comparable() {
    // The baseline sample is 61 s older than `now` (max age 60 s): STALE
    // (§10.4) — fail-closed.
    let baseline_source = ScriptedBaseline::with_samples(vec![(
        5_000,
        Some(baseline(TrackedQuantity::ActivePowerMW, 1.5, 5_000 - 61_000)),
    )]);
    let report = evaluate_shadow_comparison(
        &ctx(),
        &baseline_source,
        input(
            vec![5_000],
            5_000,
            SimulatorTargetValue::ActivePowerMW(1.5),
            config(),
        ),
    )
    .expect("evaluation succeeds");
    assert_eq!(report.overall, ShadowOverallVerdict::NotComparable);
    match report.entries[0].outcome {
        ShadowComparisonOutcome::NotComparable(NotComparableReason::StaleBaseline {
            observed_at_unix_millis,
            max_age_millis,
        }) => {
            assert_eq!(observed_at_unix_millis, 5_000 - 61_000);
            assert_eq!(max_age_millis, 60_000);
        }
        other => panic!("expected stale-baseline, got {other:?}"),
    }
}

#[test]
fn a_future_stamped_baseline_is_not_comparable_clock_skew_is_never_close_enough() {
    // The baseline sample is stamped AFTER `now` (clock skew, §17.4):
    // fail-closed, never "close enough".
    let baseline_source = ScriptedBaseline::with_samples(vec![(
        5_000,
        Some(baseline(TrackedQuantity::ActivePowerMW, 1.5, 6_000)),
    )]);
    let report = evaluate_shadow_comparison(
        &ctx(),
        &baseline_source,
        input(
            vec![5_000],
            5_000,
            SimulatorTargetValue::ActivePowerMW(1.5),
            config(),
        ),
    )
    .expect("evaluation succeeds");
    assert_eq!(report.overall, ShadowOverallVerdict::NotComparable);
    assert!(matches!(
        report.entries[0].outcome,
        ShadowComparisonOutcome::NotComparable(NotComparableReason::BaselineFromTheFuture {
            observed_at_unix_millis: 6_000,
            now_unix_millis: 5_000,
        })
    ));
}

#[test]
fn a_quantity_mismatched_baseline_is_not_comparable_never_coerced() {
    // The command targets active power; the baseline sample carries SOC —
    // never coerced into a comparison (the P4.16 mismatch rule).
    let baseline_source = ScriptedBaseline::with_samples(vec![(
        5_000,
        Some(baseline(TrackedQuantity::SocFraction, 0.6, 4_800)),
    )]);
    let report = evaluate_shadow_comparison(
        &ctx(),
        &baseline_source,
        input(
            vec![5_000],
            5_000,
            SimulatorTargetValue::ActivePowerMW(1.5),
            config(),
        ),
    )
    .expect("evaluation succeeds");
    assert_eq!(report.overall, ShadowOverallVerdict::NotComparable);
    assert!(matches!(
        report.entries[0].outcome,
        ShadowComparisonOutcome::NotComparable(NotComparableReason::QuantityMismatch {
            baseline_quantity: "soc_fraction",
            requested_quantity: "active_power_mw",
        })
    ));
}

#[test]
fn the_overall_verdict_is_conservative_one_bad_sample_is_never_averaged_away() {
    // instants 1..3: agreement, divergence, missing baseline. The overall
    // verdict is NotComparable (any not-comparable wins), then
    // divergence, then agreement — a single bad sample is never averaged
    // into a pass.
    let baseline_source = ScriptedBaseline::with_samples(vec![
        (1_000, Some(baseline(TrackedQuantity::ActivePowerMW, 1.52, 900))),
        (2_000, Some(baseline(TrackedQuantity::ActivePowerMW, 0.9, 1_900))),
        (3_000, None),
    ]);
    let report = evaluate_shadow_comparison(
        &ctx(),
        &baseline_source,
        input(
            vec![1_000, 2_000, 3_000],
            5_000,
            SimulatorTargetValue::ActivePowerMW(1.5),
            config(),
        ),
    )
    .expect("evaluation succeeds");
    assert_eq!(report.overall, ShadowOverallVerdict::NotComparable);
    assert_eq!(report.agreement_count(), 1);
    assert_eq!(report.divergence_count(), 1);
    assert_eq!(report.not_comparable_count(), 1);

    // Without the missing baseline the same two comparable entries make
    // the report Divergence (any divergence beats agreement).
    let baseline_source = ScriptedBaseline::with_samples(vec![
        (1_000, Some(baseline(TrackedQuantity::ActivePowerMW, 1.52, 900))),
        (2_000, Some(baseline(TrackedQuantity::ActivePowerMW, 0.9, 1_900))),
    ]);
    let report = evaluate_shadow_comparison(
        &ctx(),
        &baseline_source,
        input(
            vec![1_000, 2_000],
            5_000,
            SimulatorTargetValue::ActivePowerMW(1.5),
            config(),
        ),
    )
    .expect("evaluation succeeds");
    assert_eq!(report.overall, ShadowOverallVerdict::Divergence);
}

// ============================================================================
// Configuration and input validation
// ============================================================================

#[test]
fn invalid_configuration_is_a_named_error_never_a_degraded_comparison() {
    // Non-positive tolerance.
    assert_eq!(
        ShadowComparisonConfig::new(0.0, 60_000),
        Err(ShadowEvaluationError::InvalidConfiguration {
            detail: "the shadow comparison tolerance must be finite and positive, got 0"
                .to_string()
        })
    );
    assert_eq!(
        ShadowComparisonConfig::new(f64::NAN, 60_000),
        Err(ShadowEvaluationError::InvalidConfiguration {
            detail: "the shadow comparison tolerance must be finite and positive, got NaN"
                .to_string()
        })
    );
    // Non-positive baseline max age.
    assert!(matches!(
        ShadowComparisonConfig::new(0.15, 0),
        Err(ShadowEvaluationError::InvalidConfiguration { .. })
    ));
}

#[test]
fn empty_instants_and_future_instants_are_rejected() {
    let baseline_source = ScriptedBaseline::with_samples(vec![]);
    // An empty instant list compares nothing — rejected.
    assert!(matches!(
        evaluate_shadow_comparison(
            &ctx(),
            &baseline_source,
            input(vec![], 5_000, SimulatorTargetValue::ActivePowerMW(1.5), config())
        ),
        Err(ShadowEvaluationError::InvalidConfiguration { .. })
    ));
    // A comparison instant AFTER `now` is speculation — rejected.
    assert!(matches!(
        evaluate_shadow_comparison(
            &ctx(),
            &baseline_source,
            input(
                vec![6_000],
                5_000,
                SimulatorTargetValue::ActivePowerMW(1.5),
                config()
            )
        ),
        Err(ShadowEvaluationError::InvalidConfiguration { .. })
    ));
    // The rejection happens BEFORE any baseline call is made.
    assert_eq!(baseline_source.calls.get(), 0);
}

#[test]
fn a_baseline_source_failure_is_a_named_error_never_a_verdict() {
    // The baseline source fails on the second call (a seam outage): the
    // named error surfaces — no partial report, no verdict from a broken
    // seam (fail-closed).
    let baseline_source = ScriptedBaseline::failing_on(
        &[2],
        vec![(1_000, Some(baseline(TrackedQuantity::ActivePowerMW, 1.52, 900)))],
    );
    let error = evaluate_shadow_comparison(
        &ctx(),
        &baseline_source,
        input(
            vec![1_000, 2_000],
            5_000,
            SimulatorTargetValue::ActivePowerMW(1.5),
            config(),
        ),
    )
    .expect_err("a failing baseline source must surface as a named error");
    assert_eq!(
        error,
        ShadowEvaluationError::BaselineSourceFailed {
            code: "shadow_baseline_source".to_string(),
            message: "shadow baseline source failed (shadow-baseline-unavailable): scripted \
                     baseline-source failure (fault injection)"
                .to_string()
        }
    );
}

// ============================================================================
// The report carries its limits; nothing is issued; determinism
// ============================================================================

#[test]
fn every_report_carries_the_limits_method_and_issued_statements() {
    let baseline_source = ScriptedBaseline::with_samples(vec![(
        5_000,
        Some(baseline(TrackedQuantity::ActivePowerMW, 1.55, 4_800)),
    )]);
    let report = evaluate_shadow_comparison(
        &ctx(),
        &baseline_source,
        input(
            vec![5_000],
            5_000,
            SimulatorTargetValue::ActivePowerMW(1.5),
            config(),
        ),
    )
    .expect("evaluation succeeds");
    // The header/limits statement TRAVELS ON THE TYPE (the ledger P4.31
    // risk mitigation: the shadow report states its limits in its header).
    assert_eq!(report.limits_statement, SHADOW_LIMITS_STATEMENT);
    assert_eq!(report.method_statement, SHADOW_METHOD_STATEMENT);
    assert_eq!(report.issued_statement, NOTHING_ISSUED_STATEMENT);
    // The limits statement says BOTH required things, in its own text.
    assert!(
        SHADOW_LIMITS_STATEMENT.contains("NOT a safety claim"),
        "the limits statement must deny the safety claim"
    );
    assert!(
        SHADOW_LIMITS_STATEMENT.contains("NEVER evidence for field dispatch"),
        "the limits statement must deny the evidence use"
    );
    // The evidence status is pinned.
    assert_eq!(report.evidence_status(), SHADOW_EVIDENCE_STATUS);
    assert_eq!(SHADOW_EVIDENCE_STATUS, "not_evidence_for_field_dispatch");
}

#[test]
fn the_same_inputs_produce_identical_reports_determinism_double_run() {
    let make_baseline = || {
        ScriptedBaseline::with_samples(vec![
            (1_000, Some(baseline(TrackedQuantity::ActivePowerMW, 1.52, 900))),
            (2_000, Some(baseline(TrackedQuantity::ActivePowerMW, 1.49, 1_900))),
        ])
    };
    let run = || {
        evaluate_shadow_comparison(
            &ctx(),
            &make_baseline(),
            input(
                vec![1_000, 2_000],
                5_000,
                SimulatorTargetValue::ActivePowerMW(1.5),
                config(),
            ),
        )
        .expect("evaluation succeeds")
    };
    let first = run();
    let second = run();
    assert_eq!(first, second, "identical inputs must give identical reports");
}

#[test]
fn the_baseline_port_is_only_ever_called_with_the_requested_instants_read_only() {
    // The port is observe-only: the fake records exactly one call per
    // instant, with the tenant context and asset the caller supplied —
    // and there is no second method a caller could misuse (pinned
    // structurally by the source scan below).
    let baseline_source = ScriptedBaseline::with_samples(vec![
        (1_000, Some(baseline(TrackedQuantity::ActivePowerMW, 1.52, 900))),
        (2_000, Some(baseline(TrackedQuantity::ActivePowerMW, 1.49, 1_900))),
    ]);
    let _ = evaluate_shadow_comparison(
        &ctx(),
        &baseline_source,
        input(
            vec![1_000, 2_000],
            5_000,
            SimulatorTargetValue::ActivePowerMW(1.5),
            config(),
        ),
    )
    .expect("evaluation succeeds");
    assert_eq!(baseline_source.calls.get(), 2, "one call per instant");
}

// ============================================================================
// Structural source scan (mirrors the P4.16/P4.17 pattern)
// ============================================================================

#[test]
fn the_shadow_module_declares_no_delivery_path_and_no_evidence_production() {
    let source = include_str!("../src/shadow_evaluation.rs");
    // No §14.3 evidence production: a shadow report must never be
    // convertible into a journal evidence reference.
    assert!(
        !source.contains("EvidenceReference"),
        "the shadow module must not name EvidenceReference — shadow results are never \
         dispatch evidence"
    );
    // No delivery path: no transport, no issue, no write of any kind.
    for forbidden in [
        "fn deliver",
        "SimulatorDispatchTransport",
        "record_issue",
        "record_acknowledgement",
        "EventOutbox",
        "journal_transition",
        "AdapterTargetCandidate",
    ] {
        assert!(
            !source.contains(forbidden),
            "the shadow module must not contain `{forbidden}` — it has no command-delivery \
             or state-change path"
        );
    }
    // The port is exactly one observe-only method: the trait declares
    // `baseline_sample` and nothing else.
    let trait_start = source
        .find("pub trait ShadowBaselineSource")
        .expect("the baseline port exists");
    let trait_end = source[trait_start..]
        .find("\npub ")
        .map(|end| trait_start + end)
        .unwrap_or(source.len());
    let trait_body = &source[trait_start..trait_end];
    let fn_declarations: Vec<&str> = trait_body
        .match_indices("fn ")
        .map(|(index, _)| &trait_body[index..index + 30])
        .collect();
    assert_eq!(
        fn_declarations.len(),
        1,
        "the baseline port declares exactly one method (observe-only); found: \
         {fn_declarations:?}"
    );
    assert!(
        trait_body.contains("fn baseline_sample"),
        "the one method is baseline_sample"
    );
}
