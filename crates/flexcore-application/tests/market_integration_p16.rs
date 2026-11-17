// P16.21 — Market integration test suite, part 1 (the application-crate
//          seams): baseline-engine determinism, settlement-parser fixtures
//          (PJM-M29-shaped, CAISO-shaped, EBGL-shaped — ALL SYNTHETIC, no
//          real statement exists here), reconciliation outcomes + the
//          dispute-queue handoff, dispute-workflow negative tests, and the
//          per-market-grant opt-in/out negatives. WRITTEN-UNRUN.
//
// Phase 16 plan §2 row P16.21; decision record:
// `docs/phases-11-20/phase-16-market-integration-settlement/records/
// 63_p16_21_market_integration_test_suite.md`.
//
// The suite CONSUMES the wave-1 grammars, cases, and negative tests — it
// never redefines one:
//
// - baseline determinism over `flexcore_application::baseline_engine`
//   (P16.04, record 04): the replay property is an integration-level pin
//   (same inputs + same injected clock ⇒ byte-identical outcomes);
// - settlement-parser fixtures over `flexcore_application::settlement_
//   models` (P16.09, record 23): the v1 fixture grammars are the single
//   homes of the grammar rules; this suite exercises them exactly as the
//   P16.20 simulator's fixtures would arrive;
// - reconciliation outcomes over `flexcore_application::settlement_
//   reconciliation` (P16.09): Matched / AmountMismatch (both values
//   verbatim) / MissingInStatement / UnexpectedInStatement, the declared
//   tolerance's inclusive boundary, the duplicate and cross-currency
//   refusals, and the dispute-queue entries;
// - dispute negative tests over `flexcore_application::dispute_workflow`
//   (P16.12, record 26): the frozen transition table (no shortcut edges),
//   the queue-derived challenge kinds (unknown codes refuse), the 64-hex
//   chain-digest rule, the empty-bundle refusal, and the
//   resolution-without-bundle refusal;
// - the per-market-grant opt-in/out negatives over `flexcore_application::
//   bid_composition` (P16.06, record 06): an ACTIVE grant under
//   operator-approved mode IS the opt-in; a missing, revoked, expired,
//   wrong-market, or wrong-tenant grant is the opt-out — every opt-out
//   path is a named refusal, never a degraded pass.
//
// HONESTY (recorded in the decision record): every fixture value is
// SYNTHETIC and hand-built; no market is connected; no real statement has
// ever been received; no settlement number carries market meaning; no bid
// is submitted; no payment moves and no payment state is entered (the
// payment-lifecycle tests drive Agent A's store over SYNTHETIC fixtures —
// the store is in-memory test state, never a real lifecycle). Execution
// is a HUMAN GATE — per the loop bans, cargo was NOT run while authoring
// these tests; the first `cargo test -p flexcore-application --test
// market_integration_p16` run is the verification obligation (the
// WRITTEN-UNRUN class of the whole 0–10 base).
//
// WAIT-CHECK (recorded, updated at the second check): the payment-
// lifecycle (P16.14) and jurisdiction-enablement (P16.18) modules of
// wave-2 Agent A had NOT landed at the suite's first write (verified by
// directory inspection; decision record 63 §3 recorded the handoff). At
// the SECOND wait-check (batch close) they HAD landed
// (`payment_lifecycle.rs`, `jurisdiction_register.rs`, records 42/46) —
// so the two pre-committed test families were written against the LANDED
// APIs, by citation, never guessed: sections 6 (payment-lifecycle state
// machine) and 7 (jurisdiction-register enablement opt-in/out) below.

use std::collections::BTreeMap;

use flexcore_application::baseline_engine::{
    compute_baseline, BaselineOutcome, DayProfile, ProgramBaselineConfig, SdaWindowInput,
};
use flexcore_application::bid_composition::{
    compose_bid, AssetBidInput, BidCompositionError, MarketBidGrant, MarketStructureDeclaration,
    NO_BID_SUBMITTED_STATEMENT, OPERATOR_GRANT_REQUIRED_STATEMENT,
};
use flexcore_application::dispute_workflow::{
    challenge_kind_from_reason_code, transition_dispute, DisputeError, DisputeState,
    EvidenceBundle, EvidenceBundleEntry, LineItemChallenge, ResolutionKind,
    DisputeResolutionRecord, DISPUTES_REFERENCE_THE_QUEUE_STATEMENT,
};
use flexcore_application::fixed_decimal::MoneyAmount;
use flexcore_application::settlement_models::{
    extract_caiso_sqmd_row, extract_ebgl_isp_row, extract_pjm_m29_bli_csv,
    extract_pjm_m29_bli_xml, SettlementParseError, StatementXmlNode,
};
use flexcore_application::settlement_reconciliation::{
    reconcile_bli_statement, ExpectedLine, LineOutcome, Tolerance,
    EXACT_LINE_IDENTITY_STATEMENT, NO_SILENT_ADJUSTMENT_STATEMENT,
};
use flexcore_ports::fakes::FixedClock;

/// The pinned test instant (UTC; the injected clock — never a wall clock).
const PINNED_NOW_UNIX_MS: i64 = 1_760_000_000_000;

/// A 64-hex fixture digest (the P11 digest rendering length).
const FIXTURE_DIGEST: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

// ---------------------------------------------------------------------------
// 1. Baseline-engine determinism (P16.04 consumed; the integration-level
//    replay pin)
// ---------------------------------------------------------------------------

fn flat_day(day_key: &str, mw: f64, count: usize) -> DayProfile {
    DayProfile::new(
        day_key.to_string(),
        None,
        None,
        vec![mw; count],
        false,
    )
    .expect("test day profile is valid")
}

fn ten_days(interval_count: usize) -> BTreeMap<String, DayProfile> {
    let mut days = BTreeMap::new();
    for i in 1..=10 {
        let key = format!("d{i:02}");
        days.insert(key.clone(), flat_day(&key, 10.0 + i as f64, interval_count));
    }
    days
}

fn baseline_config() -> ProgramBaselineConfig {
    let mut config =
        ProgramBaselineConfig::new("tenant-1".to_string(), "program-1".to_string(), "CBL_10_IN_10".to_string())
            .expect("valid config");
    config.selection = None; // the CBL methods derive their rule from the name
    config
}

#[test]
fn baseline_engine_replay_is_byte_identical_across_calls() {
    let config = baseline_config();
    let days = ten_days(4);
    let clock_a = FixedClock::at_unix_millis(PINNED_NOW_UNIX_MS);
    let clock_b = FixedClock::at_unix_millis(PINNED_NOW_UNIX_MS);
    let outcome_a = compute_baseline(&config, &days, 4, None, &clock_a);
    let outcome_b = compute_baseline(&config, &days, 4, None, &clock_b);
    // The replay property: same inputs + same injected clock ⇒
    // byte-identical outcomes (Compare via PartialEq over the whole
    // outcome value — no float anywhere needs tolerance: every value is
    // an exact deterministic mean of the same fixtures).
    assert_eq!(outcome_a, outcome_b);
    // And the positive path is a computed profile with the PROGRAM'S
    // method name carried end to end (LR-073).
    match &outcome_a {
        BaselineOutcome::Computed(profile) => {
            assert_eq!(profile.method_name, "CBL_10_IN_10");
            assert_eq!(profile.source_day_keys.len(), 10);
        }
        BaselineOutcome::Gap(gap) => panic!("expected a computed profile, got gap {gap:?}"),
    }
}

#[test]
fn baseline_engine_sda_replay_is_byte_identical_and_symmetric() {
    // The SDA leg with the window input: capped symmetrically, both
    // ratios recorded, replay-identical.
    let mut config = ProgramBaselineConfig::new(
        "tenant-1".to_string(),
        "program-1".to_string(),
        "CBL_10_IN_10".to_string(),
    )
    .expect("valid config");
    config.sda = Some(flexcore_application::baseline_engine::SdaConfig::new(20.0).expect("valid cap"));
    let days = ten_days(4);
    // Morning window over the first two intervals: the actuals run ABOVE
    // the baseline (a positive adjustment), capped at 20%.
    let sda_window =
        SdaWindowInput::new((0, 2), vec![14.0, 14.0]).expect("valid SDA window");
    let clock = FixedClock::at_unix_millis(PINNED_NOW_UNIX_MS);
    let outcome_a = compute_baseline(&config, &days, 4, Some(&sda_window), &clock);
    let clock = FixedClock::at_unix_millis(PINNED_NOW_UNIX_MS);
    let outcome_b = compute_baseline(&config, &days, 4, Some(&sda_window), &clock);
    assert_eq!(outcome_a, outcome_b);
    match &outcome_a {
        BaselineOutcome::Computed(profile) => {
            let sda = profile.sda_applied.expect("the declared SDA was applied");
            assert!(sda.applied_ratio <= 0.20, "the applied ratio must sit at or under the declared 20% cap");
            assert!(sda.raw_ratio >= sda.applied_ratio, "a capped adjustment records the raw ratio above the applied one");
        }
        BaselineOutcome::Gap(gap) => panic!("expected a computed profile, got gap {gap:?}"),
    }
}

#[test]
fn baseline_engine_gap_rows_are_deterministic_records() {
    // An unsupported method (LR-073) is a RECORDED GAP ROW — deterministically.
    let config = ProgramBaselineConfig::new("tenant-1".to_string(), "program-1".to_string(), "DLA".to_string())
        .expect("valid config");
    let days = ten_days(4);
    let clock_a = FixedClock::at_unix_millis(PINNED_NOW_UNIX_MS);
    let clock_b = FixedClock::at_unix_millis(PINNED_NOW_UNIX_MS);
    let outcome_a = compute_baseline(&config, &days, 4, None, &clock_a);
    let outcome_b = compute_baseline(&config, &days, 4, None, &clock_b);
    assert_eq!(outcome_a, outcome_b);
    match outcome_a {
        BaselineOutcome::Gap(gap) => {
            assert_eq!(gap.method_name, "DLA");
            assert_eq!(gap.recorded_at_unix_ms, PINNED_NOW_UNIX_MS);
        }
        BaselineOutcome::Computed(_) => panic!("an unsupported method must record a gap"),
    }
}

// ---------------------------------------------------------------------------
// 2. Settlement-parser fixtures (P16.09 grammars consumed; ALL SYNTHETIC)
// ---------------------------------------------------------------------------

const SYNTHETIC_STATEMENT_REF: &str = "synthetic://m29/csv/p16-21";
const SYNTHETIC_CURRENCY: &str = "USD";

fn synthetic_pjm_csv() -> String {
    let mut text = String::from("statement_ref,charge_type\n");
    text.push_str(&format!("{SYNTHETIC_STATEMENT_REF},ENERGY_CHARGE,100.00,USD\n"));
    text.push_str(&format!("{SYNTHETIC_STATEMENT_REF},CAPACITY_CHARGE,50.50,USD\n"));
    text.push_str(&format!("{SYNTHETIC_STATEMENT_REF},ANCILLARY_CHARGE,25.25,USD\n"));
    text
}

fn synthetic_pjm_items() -> Vec<flexcore_application::settlement_models::PjmBliLineItem> {
    extract_pjm_m29_bli_csv(&synthetic_pjm_csv()).expect("the synthetic statement parses")
}

#[test]
fn pjm_m29_csv_fixture_extracts_three_synthetic_lines() {
    let items = synthetic_pjm_items();
    assert_eq!(items.len(), 3);
    assert_eq!(items[0].line_ref, format!("{SYNTHETIC_STATEMENT_REF}#0"));
    assert_eq!(items[0].charge_type, "ENERGY_CHARGE");
    assert_eq!(items[0].amount.canonical_string(), "100.00 USD");
    assert_eq!(items[2].amount.canonical_string(), "25.25 USD");
    assert_eq!(items[0].statement_ref, SYNTHETIC_STATEMENT_REF);
}

#[test]
fn pjm_m29_csv_fixture_refusals_are_named() {
    // Wrong cell count: outside the v1 grammar — refused, never guessed.
    let err = extract_pjm_m29_bli_csv("statement_ref,charge_type\nR,X,1.00").expect_err("3 cells refused");
    assert_eq!(err.name(), "SETTLEMENT_PARSE_STRUCTURE_UNRECOGNIZED");
    // A quoted cell: outside the v1 subset (no quoted-cell grammar, no
    // escaped commas). Here the quote characters ride INTO the cells: the
    // row still has four cells, so the amount fails the exact-decimal
    // parse — a named refusal either way, never an approximation.
    let err = extract_pjm_m29_bli_csv(
        "statement_ref,charge_type\n\"R\",\"X\",\"1.00\",\"USD\"",
    )
    .expect_err("quoted cells refused");
    assert_eq!(err.name(), "SETTLEMENT_PARSE_DECIMAL_INVALID");
    // Header-only: an empty statement is a refusal, never a clean zero bill.
    let err = extract_pjm_m29_bli_csv("statement_ref,charge_type\n").expect_err("header-only refused");
    assert_eq!(err.name(), "SETTLEMENT_PARSE_STRUCTURE_UNRECOGNIZED");
    // A bad decimal: named.
    let err = extract_pjm_m29_bli_csv(&format!("{SYNTHETIC_STATEMENT_REF},X,not-a-decimal,USD"))
        .expect_err("bad decimal refused");
    assert_eq!(err.name(), "SETTLEMENT_PARSE_DECIMAL_INVALID");
}

#[test]
fn pjm_m29_xml_fixture_extracts_and_refuses() {
    let node = |lines: Vec<(&str, &str, &str)>| {
        let children = lines
            .into_iter()
            .map(|(line_ref, charge_type, amount)| StatementXmlNode {
                name: "bli".to_string(),
                attrs: vec![
                    ("ref".to_string(), line_ref.to_string()),
                    ("charge_type".to_string(), charge_type.to_string()),
                ],
                text: Some(amount.to_string()),
                children: Vec::new(),
            })
            .collect();
        StatementXmlNode {
            name: "statement".to_string(),
            attrs: vec![
                ("ref".to_string(), SYNTHETIC_STATEMENT_REF.to_string()),
                ("currency".to_string(), SYNTHETIC_CURRENCY.to_string()),
            ],
            text: None,
            children,
        }
    };
    let items = extract_pjm_m29_bli_xml(&node(vec![
        ("BLI-1", "ENERGY_CHARGE", "100.00"),
        ("BLI-2", "CAPACITY_CHARGE", "50.50"),
    ]))
    .expect("valid statement");
    assert_eq!(items.len(), 2);
    assert_eq!(items[0].line_ref, "BLI-1");
    assert_eq!(items[1].amount.canonical_string(), "50.50 USD");
    // A wrong root element is outside the grammar.
    let mut wrong_root = node(vec![("BLI-1", "X", "1.00")]);
    wrong_root.name = "invoice".to_string();
    let err = extract_pjm_m29_bli_xml(&wrong_root).expect_err("wrong root refused");
    assert_eq!(err.name(), "SETTLEMENT_PARSE_STRUCTURE_UNRECOGNIZED");
}

#[test]
fn caiso_sqmd_fixture_validates_the_interval_exactly() {
    // Exactly 5 minutes: the record constructs.
    let record =
        extract_caiso_sqmd_row("1000,300100,425.0,kWh,ESTIMATED").expect("valid SQMD row");
    assert_eq!(record.quantity_basis, "kWh");
    assert_eq!(record.quality_flag, "ESTIMATED");
    assert_eq!(record.meter_quantity.canonical_string(), "425.0");
    // Any other length is REFUSED — re-binning is refused, never performed.
    let err = extract_caiso_sqmd_row("1000,450100,425.0,kWh,ESTIMATED")
        .expect_err("a non-5-minute interval is refused");
    assert_eq!(err.name(), "SETTLEMENT_PARSE_INTERVAL_INVALID");
}

#[test]
fn ebgl_isp_fixture_validates_the_interval_exactly_and_derives_direction() {
    // Exactly 15 minutes; a positive imbalance is SURPLUS, a negative one
    // DEFICIT under the DECLARED sign convention.
    let surplus = extract_ebgl_isp_row("1000,901000,900.0").expect("valid ISP row");
    assert_eq!(surplus.imbalance_energy_mwh.canonical_string(), "900.0");
    let deficit = extract_ebgl_isp_row("901000,1802000,-125.5").expect("valid ISP row");
    assert_eq!(
        deficit.imbalance_energy_mwh.canonical_string(),
        "-125.5"
    );
    // A 60-minute pre-transition ISP is REFUSED — never re-binned.
    let err = extract_ebgl_isp_row("1000,3601000,900.0").expect_err("60-minute ISP refused");
    assert_eq!(err.name(), "SETTLEMENT_PARSE_INTERVAL_INVALID");
}

// ---------------------------------------------------------------------------
// 3. Reconciliation outcomes + the dispute-queue handoff (P16.09 consumed)
// ---------------------------------------------------------------------------

fn expected_lines(cents: &[i64]) -> Vec<ExpectedLine> {
    cents
        .iter()
        .enumerate()
        .map(|(index, cents)| {
            let line_ref = format!("{SYNTHETIC_STATEMENT_REF}#{index}");
            let amount = MoneyAmount::parse(&render_cents(*cents), SYNTHETIC_CURRENCY.to_string())
                .expect("valid fixture amount");
            ExpectedLine::new(line_ref, amount).expect("valid expected line")
        })
        .collect()
}

/// Integer cents → decimal text (the same exact-integer rendering the
/// simulator's fixture generator uses; no float anywhere in this suite).
fn render_cents(cents: i64) -> String {
    let sign = if cents < 0 { "-" } else { "" };
    let magnitude = cents.abs();
    format!("{sign}{}.{:02}", magnitude / 100, magnitude % 100)
}

#[test]
fn reconciliation_outcomes_cover_all_four_kinds() {
    // The expected set carries THREE lines; the statement carries only
    // the first two PLUS one unexpected line — so all four outcome kinds
    // appear in one run (Matched, MissingInStatement,
    // UnexpectedInStatement; the fourth kind — AmountMismatch — is its
    // own test below).
    let expected = expected_lines(&[100_00, 50_50, 25_25]);
    let mut items = synthetic_pjm_items();
    // Drop the last statement line (its expected counterpart becomes
    // MissingInStatement) and add an unexpected one.
    items.pop();
    items.push(flexcore_application::settlement_models::PjmBliLineItem {
        line_ref: format!("{SYNTHETIC_STATEMENT_REF}#unexpected"),
        charge_type: "SURPRISE_CHARGE".to_string(),
        amount: MoneyAmount::parse("1.00", SYNTHETIC_CURRENCY.to_string()).expect("valid"),
        statement_ref: SYNTHETIC_STATEMENT_REF.to_string(),
    });
    let report = reconcile_bli_statement(SYNTHETIC_STATEMENT_REF, &expected, &items, Tolerance::Exact)
        .expect("valid reconciliation");
    assert_eq!(report.outcomes.get(&format!("{SYNTHETIC_STATEMENT_REF}#0")), Some(&LineOutcome::Matched));
    assert_eq!(report.outcomes.get(&format!("{SYNTHETIC_STATEMENT_REF}#1")), Some(&LineOutcome::Matched));
    assert!(matches!(
        report.outcomes.get(&format!("{SYNTHETIC_STATEMENT_REF}#2")),
        Some(LineOutcome::MissingInStatement)
    ));
    assert!(matches!(
        report.outcomes.get(&format!("{SYNTHETIC_STATEMENT_REF}#unexpected")),
        Some(LineOutcome::UnexpectedInStatement)
    ));
    // The dispute queue carries one entry per non-matched outcome, both
    // values verbatim where both exist.
    assert_eq!(report.dispute_queue.len(), 2);
    assert_eq!(report.matched_count, 2);
    assert_eq!(report.mismatch_count, 2);
    // NO_SILENT_ADJUSTMENT: the engine's only output is evidence.
    assert!(NO_SILENT_ADJUSTMENT_STATEMENT.contains("dispute-queue entry"));
    assert!(EXACT_LINE_IDENTITY_STATEMENT.contains("EXACT string equality"));
}

#[test]
fn reconciliation_amount_mismatch_carries_both_values_verbatim() {
    let expected = expected_lines(&[100_00]);
    let items = extract_pjm_m29_bli_csv(&format!(
        "statement_ref,charge_type\n{SYTHETIC_STATEMENT_REF},ENERGY_CHARGE,101.25,USD\n"
    ))
    .expect("valid statement");
    let report = reconcile_bli_statement(SYNTHETIC_STATEMENT_REF, &expected, &items, Tolerance::Exact)
        .expect("valid reconciliation");
    let Some(LineOutcome::AmountMismatch { expected, stated, deviation }) =
        report.outcomes.get(&format!("{SYNTHETIC_STATEMENT_REF}#0"))
    else {
        panic!("expected an AmountMismatch outcome");
    };
    assert_eq!(expected.canonical_string(), "100.00 USD");
    assert_eq!(stated.canonical_string(), "101.25 USD");
    assert_eq!(deviation.canonical_string(), "1.25 USD");
}

#[test]
fn reconciliation_tolerance_boundary_is_inclusive() {
    // Exact: a 1-cent deviation is a mismatch.
    let expected = expected_lines(&[100_00]);
    let items = extract_pjm_m29_bli_csv(&format!(
        "statement_ref,charge_type\n{SYNTHETIC_STATEMENT_REF},ENERGY_CHARGE,100.01,USD\n"
    ))
    .expect("valid statement");
    let report = reconcile_bli_statement(SYNTHETIC_STATEMENT_REF, &expected, &items, Tolerance::Exact)
        .expect("valid reconciliation");
    assert_eq!(report.mismatch_count, 1);
    // Within 0.01: the SAME deviation is NOT a mismatch (inclusive
    // boundary — a deviation exactly equal to the tolerance matches).
    let bound = flexcore_application::fixed_decimal::FixedDecimal::parse("0.01").expect("valid bound");
    let report = reconcile_bli_statement(
        SYNTHETIC_STATEMENT_REF,
        &expected,
        &items,
        Tolerance::within(bound),
    )
    .expect("valid reconciliation");
    assert_eq!(report.matched_count, 1);
    assert_eq!(report.mismatch_count, 0);
}

#[test]
fn reconciliation_run_level_refusals_are_named() {
    // Duplicate identity on the expected side refuses (never a silent
    // merge) — and duplicate identity on the stated side too.
    let doubled = vec![
        ExpectedLine::new("dup".to_string(), MoneyAmount::parse("1.00", "USD".to_string()).expect("valid")).expect("valid"),
        ExpectedLine::new("dup".to_string(), MoneyAmount::parse("2.00", "USD".to_string()).expect("valid")).expect("valid"),
    ];
    let err = reconcile_bli_statement(SYNTHETIC_STATEMENT_REF, &doubled, &synthetic_pjm_items(), Tolerance::Exact)
        .expect_err("duplicate identity refuses");
    assert_eq!(err.name(), "RECON_DUPLICATE_LINE_REF");
    // Empty sets refuse (fail-closed).
    let err = reconcile_bli_statement(SYNTHETIC_STATEMENT_REF, &[], &synthetic_pjm_items(), Tolerance::Exact)
        .expect_err("empty expected set refuses");
    assert_eq!(err.name(), "RECON_EMPTY_EXPECTED_SET");
    // Cross-currency on the same line is a run-level refusal.
    let mixed = vec![ExpectedLine::new(
        format!("{SYNTHETIC_STATEMENT_REF}#0"),
        MoneyAmount::parse("100.00", "EUR".to_string()).expect("valid"),
    )
    .expect("valid")];
    let err = reconcile_bli_statement(SYNTHETIC_STATEMENT_REF, &mixed, &synthetic_pjm_items(), Tolerance::Exact)
        .expect_err("cross-currency refuses");
    assert_eq!(err.name(), "RECON_CURRENCY_MISMATCH");
}

// ---------------------------------------------------------------------------
// 4. Dispute-workflow negative tests (P16.12 consumed)
// ---------------------------------------------------------------------------

#[test]
fn dispute_negative_frozen_table_has_no_shortcut_edges() {
    // The recorded edges open.
    assert!(transition_dispute(DisputeState::Opened, DisputeState::EvidenceAssembled).is_ok());
    assert!(transition_dispute(DisputeState::EvidenceAssembled, DisputeState::Submitted).is_ok());
    assert!(transition_dispute(DisputeState::Submitted, DisputeState::Resolved).is_ok());
    // The shortcuts REFUSE.
    let err = transition_dispute(DisputeState::Opened, DisputeState::Resolved)
        .expect_err("Opened→Resolved must refuse");
    assert_eq!(err.name(), "DISPUTE_INVALID_TRANSITION");
    let err = transition_dispute(DisputeState::Opened, DisputeState::Submitted)
        .expect_err("Opened→Submitted must refuse");
    assert!(matches!(err, DisputeError::InvalidTransition { .. }));
    let err = transition_dispute(DisputeState::EvidenceAssembled, DisputeState::Resolved)
        .expect_err("a resolution requires submission");
    assert!(matches!(err, DisputeError::InvalidTransition { .. }));
    // Terminal states stay terminal.
    for from in [DisputeState::Resolved, DisputeState::Withdrawn] {
        let err = transition_dispute(from, DisputeState::Opened).expect_err("terminal stays terminal");
        assert!(matches!(err, DisputeError::InvalidTransition { .. }));
    }
}

#[test]
fn dispute_negative_unknown_reason_codes_refuse() {
    assert!(matches!(
        challenge_kind_from_reason_code("LINE_AMOUNT_MISMATCH"),
        Ok(_)
    ));
    assert!(matches!(
        challenge_kind_from_reason_code("LINE_MISSING_IN_STATEMENT"),
        Ok(_)
    ));
    assert!(matches!(
        challenge_kind_from_reason_code("LINE_UNEXPECTED_IN_STATEMENT"),
        Ok(_)
    ));
    // An unknown code is a named refusal — never a generic "other".
    let err = challenge_kind_from_reason_code("SOMETHING_ELSE").expect_err("unknown code refuses");
    assert_eq!(err.name(), "DISPUTE_UNRECOGNIZED_REASON_CODE");
    assert!(DISPUTES_REFERENCE_THE_QUEUE_STATEMENT.contains("unconstructable"));
}

#[test]
fn dispute_negative_chain_digest_rules() {
    // Exactly 64 hex characters or refused.
    let err = EvidenceBundleEntry::new(
        "e1".to_string(),
        "evidence://1".to_string(),
        "market".to_string(),
        "short".to_string(),
    )
    .expect_err("a short digest refuses");
    assert_eq!(err.name(), "DISPUTE_INVALID_CHAIN_DIGEST_REF");
    let err = EvidenceBundleEntry::new(
        "e1".to_string(),
        "evidence://1".to_string(),
        "market".to_string(),
        &format!("{FIXTURE_DIGEST}g"), // 64 chars, but 'g' is not hex
    )
    .expect_err("a non-hex digest refuses");
    assert_eq!(err.name(), "DISPUTE_INVALID_CHAIN_DIGEST_REF");
    // An EMPTY bundle refuses — no evidence, no argument.
    let err = EvidenceBundle::assemble("bundle://1".to_string(), "challenge://1".to_string(), Vec::new())
        .expect_err("an empty bundle refuses");
    assert_eq!(err.name(), "DISPUTE_EMPTY_REFERENCE");
}

#[test]
fn dispute_negative_resolution_requires_a_bundle() {
    let err = DisputeResolutionRecord::new(
        "resolution://1".to_string(),
        "challenge://1".to_string(),
        String::new(),
        ResolutionKind::Upheld,
        "operator-1".to_string(),
        String::new(),
    )
    .expect_err("a resolution without a bundle refuses");
    assert_eq!(err.name(), "DISPUTE_RESOLUTION_WITHOUT_BUNDLE");
    // And the challenge itself requires its queue linkage.
    let err = LineItemChallenge::new(
        String::new(),
        "queue://1".to_string(),
        "line://1".to_string(),
        "stmt://1".to_string(),
        flexcore_application::dispute_workflow::ChallengeKind::AmountMismatch,
        "operator-1".to_string(),
    )
    .expect_err("a challenge without its reference refuses");
    assert_eq!(err.name(), "DISPUTE_EMPTY_REFERENCE");
}

// ---------------------------------------------------------------------------
// 5. Per-market-grant opt-in/out negatives (the P16.06 model consumed;
//    an ACTIVE grant under operator-approved mode IS the opt-in)
// ---------------------------------------------------------------------------

fn opt_in_grant() -> MarketBidGrant {
    MarketBidGrant::new(
        "grant://p16-21/1".to_string(),
        "tenant-1".to_string(),
        "PJM_SINGLE_AGGREGATION_NODE".to_string(),
        "ACTIVE".to_string(),
        "operator-1".to_string(),
        PINNED_NOW_UNIX_MS,
        PINNED_NOW_UNIX_MS + 3_600_000,
        "audit://p16-21/1".to_string(),
    )
    .expect("valid grant")
}

fn asset_inputs() -> Vec<AssetBidInput> {
    vec![AssetBidInput {
        asset_id: "asset-1".to_string(),
        envelope_ref: "envelope://asset-1@v1".to_string(),
        interval_quantities_mw: vec![1.0, 2.0],
    }]
}

fn interval_bounds() -> Vec<(i64, i64)> {
    vec![(1_760_000_000_000, 1_760_000_300_000), (1_760_000_300_000, 1_760_000_600_000)]
}

#[test]
fn opt_in_path_composes_the_record_and_never_submits() {
    let grant = opt_in_grant();
    let structure = MarketStructureDeclaration::PjmSingleAggregationNode {
        aggregation_node_ref: "node-1".to_string(),
    };
    let clock = FixedClock::at_unix_millis(PINNED_NOW_UNIX_MS);
    let record = compose_bid(
        &structure,
        &grant,
        "operator-approved",
        "tenant-1",
        &interval_bounds(),
        &asset_inputs(),
        &clock,
    )
    .expect("the opt-in path composes");
    match &record {
        flexcore_application::bid_composition::ComposedBidRecord::PjmAggregationBid {
            intervals,
            ..
        } => assert_eq!(intervals.len(), 2),
        other => panic!("expected the PJM aggregation record, got {other:?}"),
    }
    // NO_BID_SUBMITTED: the output is an internal record — none can be.
    assert!(NO_BID_SUBMITTED_STATEMENT.contains("none can be"));
    assert!(OPERATOR_GRANT_REQUIRED_STATEMENT.contains("operator-approved"));
}

#[test]
fn opt_out_paths_are_all_named_refusals() {
    let structure = MarketStructureDeclaration::PjmSingleAggregationNode {
        aggregation_node_ref: "node-1".to_string(),
    };
    let clock = FixedClock::at_unix_millis(PINNED_NOW_UNIX_MS);
    // Opt-out case 1: the grant is REVOKED.
    let grant = MarketBidGrant::new(
        "grant://p16-21/2".to_string(),
        "tenant-1".to_string(),
        "PJM_SINGLE_AGGREGATION_NODE".to_string(),
        "REVOKED".to_string(),
        "operator-1".to_string(),
        PINNED_NOW_UNIX_MS,
        PINNED_NOW_UNIX_MS + 3_600_000,
        "audit://p16-21/2".to_string(),
    )
    .expect("valid grant");
    let err = compose_bid(&structure, &grant, "operator-approved", "tenant-1", &interval_bounds(), &asset_inputs(), &clock)
        .expect_err("a revoked grant is the opt-out");
    assert!(matches!(err, BidCompositionError::GrantNotActive { .. }));
    // Opt-out case 2: the grant has EXPIRED (end-exclusive validity).
    let grant = MarketBidGrant::new(
        "grant://p16-21/3".to_string(),
        "tenant-1".to_string(),
        "PJM_SINGLE_AGGREGATION_NODE".to_string(),
        "ACTIVE".to_string(),
        "operator-1".to_string(),
        PINNED_NOW_UNIX_MS - 7_200_000,
        PINNED_NOW_UNIX_MS - 3_600_000,
        "audit://p16-21/3".to_string(),
    )
    .expect("valid grant");
    let err = compose_bid(&structure, &grant, "operator-approved", "tenant-1", &interval_bounds(), &asset_inputs(), &clock)
        .expect_err("an expired grant is the opt-out");
    assert!(matches!(err, BidCompositionError::GrantExpired { .. }));
    // Opt-out case 3: the grant covers a DIFFERENT market.
    let grant = MarketBidGrant::new(
        "grant://p16-21/4".to_string(),
        "tenant-1".to_string(),
        "CAISO_PDR_TWO_ID_LOAD_SHIFT".to_string(),
        "ACTIVE".to_string(),
        "operator-1".to_string(),
        PINNED_NOW_UNIX_MS,
        PINNED_NOW_UNIX_MS + 3_600_000,
        "audit://p16-21/4".to_string(),
    )
    .expect("valid grant");
    let err = compose_bid(&structure, &grant, "operator-approved", "tenant-1", &interval_bounds(), &asset_inputs(), &clock)
        .expect_err("a wrong-market grant is the opt-out");
    assert!(matches!(err, BidCompositionError::GrantMarketMismatch { .. }));
    // Opt-out case 4: the grant belongs to ANOTHER TENANT.
    let grant = MarketBidGrant::new(
        "grant://p16-21/5".to_string(),
        "tenant-other".to_string(),
        "PJM_SINGLE_AGGREGATION_NODE".to_string(),
        "ACTIVE".to_string(),
        "operator-1".to_string(),
        PINNED_NOW_UNIX_MS,
        PINNED_NOW_UNIX_MS + 3_600_000,
        "audit://p16-21/5".to_string(),
    )
    .expect("valid grant");
    let err = compose_bid(&structure, &grant, "operator-approved", "tenant-1", &interval_bounds(), &asset_inputs(), &clock)
        .expect_err("a foreign-tenant grant is the opt-out");
    assert!(matches!(err, BidCompositionError::GrantTenantMismatch { .. }));
    // Opt-out case 5: the control mode is not operator-approved.
    let grant = opt_in_grant();
    for mode in ["simulation", "read-only-shadow", "bounded-automation"] {
        let err = compose_bid(&structure, &grant, mode, "tenant-1", &interval_bounds(), &asset_inputs(), &clock)
            .expect_err("any other mode is the opt-out");
        assert!(matches!(err, BidCompositionError::ControlModeInsufficient { .. }));
    }
    // Opt-out case 6: the quantities lost their envelope provenance (no
    // provenance, no bid — the envelope is the truth layer).
    let inputs = vec![AssetBidInput {
        asset_id: "asset-1".to_string(),
        envelope_ref: String::new(),
        interval_quantities_mw: vec![1.0, 2.0],
    }];
    let err = compose_bid(&structure, &grant, "operator-approved", "tenant-1", &interval_bounds(), &inputs, &clock)
        .expect_err("missing envelope provenance refuses");
    assert!(matches!(err, BidCompositionError::MissingEnvelopeProvenance { .. }));
}

// ---------------------------------------------------------------------------
// 6. Payment-lifecycle state machine (P16.14, Agent A's landed module —
//    consumed by citation, never redefined; every fixture value is
//    SYNTHETIC and no payment exists or moves anywhere)
// ---------------------------------------------------------------------------

use flexcore_application::payment_lifecycle::{
    try_payment_transition, PaymentLifecycleError, PaymentLifecycleStore, PaymentState,
    PaymentTransitionRecord,
};
use flexcore_domain::market_program_model::AuditChainLink;

const SYNTHETIC_TENANT: &str = "tenant-1";
const SYNTHETIC_ASSET: &str = "asset-1";
const SYNTHETIC_PROGRAM: &str = "program-1";
const SYNTHETIC_CURRENCY_PAYMENT: &str = "USD";

fn synthetic_amount(cents: i64) -> MoneyAmount {
    MoneyAmount::parse(&render_cents(cents), SYNTHETIC_CURRENCY_PAYMENT.to_string())
        .expect("valid fixture amount")
}

fn synthetic_chain_link(id: &str) -> AuditChainLink {
    AuditChainLink::new(id.to_string()).expect("valid fixture link")
}

fn synthetic_transition(
    transition_id: &str,
    from: PaymentState,
    to: PaymentState,
    basis_ref: &str,
    amount: Option<MoneyAmount>,
    previous: Option<&str>,
) -> PaymentTransitionRecord {
    PaymentTransitionRecord::new(
        transition_id.to_string(),
        SYNTHETIC_TENANT.to_string(),
        SYNTHETIC_ASSET.to_string(),
        SYNTHETIC_PROGRAM.to_string(),
        from,
        to,
        basis_ref.to_string(),
        amount,
        synthetic_chain_link("audit://p16-21/payment"),
        previous.map(|s| s.to_string()),
        PINNED_NOW_UNIX_MS,
        "operator-1".to_string(),
    )
    .expect("valid fixture transition")
}

#[test]
fn payment_lifecycle_frozen_table_has_exactly_two_edges() {
    // The frozen edges open.
    assert!(try_payment_transition(PaymentState::Forecasted, PaymentState::Earned).is_ok());
    assert!(try_payment_transition(PaymentState::Earned, PaymentState::Paid).is_ok());
    // NO FORECASTED → PAID shortcut: a payment without a recorded earned
    // basis is unrepresentable.
    let err = try_payment_transition(PaymentState::Forecasted, PaymentState::Paid)
        .expect_err("the FORECASTED→PAID shortcut must refuse");
    assert_eq!(err.name(), "PAYMENT_INVALID_TRANSITION");
    // No backwards edges.
    let err = try_payment_transition(PaymentState::Earned, PaymentState::Forecasted)
        .expect_err("no backwards edges");
    assert_eq!(err.name(), "PAYMENT_INVALID_TRANSITION");
    // PAID is terminal.
    assert!(PaymentState::Paid.is_terminal());
    let err = try_payment_transition(PaymentState::Paid, PaymentState::Earned)
        .expect_err("PAID stays terminal");
    assert_eq!(err.name(), "PAYMENT_INVALID_TRANSITION");
}

#[test]
fn payment_lifecycle_chain_forecasted_earned_paid_records_every_state() {
    let mut store = PaymentLifecycleStore::new();
    store
        .open_lifecycle(
            SYNTHETIC_TENANT.to_string(),
            SYNTHETIC_ASSET.to_string(),
            SYNTHETIC_PROGRAM.to_string(),
            synthetic_amount(50_000),
            synthetic_chain_link("audit://p16-21/open"),
            PINNED_NOW_UNIX_MS,
            "operator-1".to_string(),
        )
        .expect("the synthetic lifecycle opens");
    assert_eq!(
        store.state_of(SYNTHETIC_TENANT, SYNTHETIC_ASSET, SYNTHETIC_PROGRAM),
        Some(PaymentState::Forecasted)
    );
    // FORECASTED → EARNED against a named basis (exact decimal, same
    // currency as the chain).
    store
        .apply_transition(synthetic_transition(
            "t1",
            PaymentState::Forecasted,
            PaymentState::Earned,
            "statement://p16-21/line-1",
            Some(synthetic_amount(42_000)),
            None,
        ))
        .expect("the earned transition applies");
    assert_eq!(
        store.state_of(SYNTHETIC_TENANT, SYNTHETIC_ASSET, SYNTHETIC_PROGRAM),
        Some(PaymentState::Earned)
    );
    // EARNED → PAID against a named payment basis (terminal).
    store
        .apply_transition(synthetic_transition(
            "t2",
            PaymentState::Earned,
            PaymentState::Paid,
            "payment://p16-21/1",
            Some(synthetic_amount(42_000)),
            Some("t1"),
        ))
        .expect("the paid transition applies");
    assert_eq!(
        store.state_of(SYNTHETIC_TENANT, SYNTHETIC_ASSET, SYNTHETIC_PROGRAM),
        Some(PaymentState::Paid)
    );
    // The chain is append-order and exact.
    let chain = store
        .chain_of(SYNTHETIC_TENANT, SYNTHETIC_ASSET, SYNTHETIC_PROGRAM)
        .expect("the chain exists");
    assert_eq!(chain.len(), 2);
    assert_eq!(chain[0].transition_id(), "t1");
    assert_eq!(chain[1].previous_transition_id(), Some("t1"));
    // NO payment exists anywhere: these are in-memory SYNTHETIC fixtures.
    assert!(flexcore_application::payment_lifecycle::NO_PAYMENT_RECORD_EXISTS_STATEMENT.contains("No payment record exists"));
}

#[test]
fn payment_lifecycle_negatives_are_all_named() {
    let mut store = PaymentLifecycleStore::new();
    // No lifecycle: a transition refuses (LifecycleNotFound).
    let err = store
        .apply_transition(synthetic_transition(
            "t0",
            PaymentState::Forecasted,
            PaymentState::Earned,
            "statement://x",
            None,
            None,
        ))
        .expect_err("no lifecycle refuses");
    assert_eq!(err.name(), "PAYMENT_LIFECYCLE_NOT_FOUND");
    // Open the lifecycle (exactly one per tenant/asset/program).
    store
        .open_lifecycle(
            SYNTHETIC_TENANT.to_string(),
            SYNTHETIC_ASSET.to_string(),
            SYNTHETIC_PROGRAM.to_string(),
            synthetic_amount(50_000),
            synthetic_chain_link("audit://p16-21/open"),
            PINNED_NOW_UNIX_MS,
            "operator-1".to_string(),
        )
        .expect("opens once");
    let err = store
        .open_lifecycle(
            SYNTHETIC_TENANT.to_string(),
            SYNTHETIC_ASSET.to_string(),
            SYNTHETIC_PROGRAM.to_string(),
            synthetic_amount(1),
            synthetic_chain_link("audit://p16-21/open-2"),
            PINNED_NOW_UNIX_MS,
            "operator-1".to_string(),
        )
        .expect_err("a second lifecycle for the same key refuses");
    assert_eq!(err.name(), "PAYMENT_LIFECYCLE_ALREADY_OPEN");
    // A missing earned basis refuses (a state is never entered without
    // the record that justifies it).
    let err = store
        .apply_transition(synthetic_transition(
            "t1",
            PaymentState::Forecasted,
            PaymentState::Earned,
            "   ",
            None,
            None,
        ))
        .expect_err("an empty earned basis refuses");
    assert_eq!(err.name(), "PAYMENT_EARNED_BASIS_REQUIRED");
    // A broken chain head refuses (fabricated chain, named).
    let err = store
        .apply_transition(synthetic_transition(
            "t1",
            PaymentState::Forecasted,
            PaymentState::Earned,
            "statement://p16-21/line-1",
            None,
            Some("fabricated-head"),
        ))
        .expect_err("a fabricated chain head refuses");
    assert_eq!(err.name(), "PAYMENT_CHAIN_LINK_MISMATCH");
    // The correct first transition applies, then a currency change
    // mid-chain refuses (money of two currencies is never mixed).
    store
        .apply_transition(synthetic_transition(
            "t1",
            PaymentState::Forecasted,
            PaymentState::Earned,
            "statement://p16-21/line-1",
            Some(synthetic_amount(42_000)),
            None,
        ))
        .expect("applies");
    let eur = MoneyAmount::parse("1.00", "EUR".to_string()).expect("valid");
    let err = store
        .apply_transition(synthetic_transition(
            "t2",
            PaymentState::Earned,
            PaymentState::Paid,
            "payment://p16-21/1",
            Some(eur),
            Some("t1"),
        ))
        .expect_err("a currency change mid-chain refuses");
    assert_eq!(err.name(), "PAYMENT_CURRENCY_MISMATCH_IN_CHAIN");
}

// ---------------------------------------------------------------------------
// 7. Jurisdiction-register enablement opt-in/out negatives (P16.18,
//    Agent A's landed module — consumed by citation; NONE enabled by
//    default; the ENABLED_BY_D1 path is the ONLY opt-in and it requires
//    the human owner's decision reference + the P11 audit link)
// ---------------------------------------------------------------------------

use flexcore_application::jurisdiction_register::{
    JurisdictionEntry, MarketJurisdictionRegister, MarketPosture, RegisterError,
    TenantMarketEnablement, NONE_ENABLED_BY_DEFAULT_STATEMENT,
};
use flexcore_application::capacity_market_records::SourcedDate;

fn synthetic_timeline() -> Vec<TimelineEntry> {
    let date = SourcedDate::new(
        "2026-11-01".to_string(),
        "synthetic fixture source (no market meaning)".to_string(),
        "2026-11-07".to_string(),
        true,
    )
    .expect("the re-verify rule is mandatory");
    vec![TimelineEntry::new("fixture".to_string(), date, None).expect("valid entry")]
}

#[test]
fn enablement_none_is_enabled_by_default_and_only_d1_enables() {
    let mut register = MarketJurisdictionRegister::new();
    register
        .register_jurisdiction(
            JurisdictionEntry::new(
                "fixture-iso".to_string(),
                "Fixture ISO (synthetic)".to_string(),
                "fixture participation model".to_string(),
                MarketPosture::Ferc2222,
                synthetic_timeline(),
            )
            .expect("valid jurisdiction row"),
        )
        .expect("registers once");
    // NONE enabled by default.
    assert!(NONE_ENABLED_BY_DEFAULT_STATEMENT.contains("ZERO"));
    assert!(register.enabled_jurisdictions_for_tenant(SYNTHETIC_TENANT).is_empty());
    // The opt-in is the ONLY construction path requiring the decision
    // reference AND the audit link: an empty decision reference refuses.
    let err = TenantMarketEnablement::enabled_by_d1(
        SYNTHETIC_TENANT.to_string(),
        "fixture-iso".to_string(),
        String::new(),
        synthetic_chain_link("audit://p16-21/d1"),
        PINNED_NOW_UNIX_MS,
    )
    .expect_err("an ENABLED_BY_D1 row without its decision reference refuses");
    assert!(matches!(err, RegisterError::D1DecisionRefRequired));
    // The full opt-in path: decision reference + audit link + a known
    // jurisdiction.
    let enablement = TenantMarketEnablement::enabled_by_d1(
        SYNTHETIC_TENANT.to_string(),
        "fixture-iso".to_string(),
        "d1://synthetic-fixture-decision".to_string(),
        synthetic_chain_link("audit://p16-21/d1"),
        PINNED_NOW_UNIX_MS,
    )
    .expect("the opt-in constructs with its decision and audit link");
    register.record_enablement(enablement).expect("records");
    assert_eq!(
        register.enabled_jurisdictions_for_tenant(SYNTHETIC_TENANT),
        vec!["fixture-iso"]
    );
    assert!(register.validate().is_ok());
    // The opt-out states are recorded without any decision: DISABLED and
    // PENDING_D1_DECISION carry no market reach.
    for (state, expected_name) in [
        (TenantMarketEnablement::disabled(SYNTHETIC_TENANT.to_string(), "fixture-iso".to_string(), PINNED_NOW_UNIX_MS), "DISABLED"),
        (TenantMarketEnablement::pending_d1_decision(SYNTHETIC_TENANT.to_string(), "fixture-iso".to_string(), PINNED_NOW_UNIX_MS), "PENDING_D1_DECISION"),
    ] {
        let row = state.expect("valid opt-out row");
        assert_eq!(row.state().name(), expected_name);
        assert!(row.decision_ref().is_none());
        assert!(row.audit_chain_link().is_none());
    }
}

#[test]
fn enablement_negatives_are_all_named() {
    let mut register = MarketJurisdictionRegister::new();
    // An enablement for an UNKNOWN jurisdiction refuses.
    let enablement = TenantMarketEnablement::enabled_by_d1(
        SYNTHETIC_TENANT.to_string(),
        "unknown-iso".to_string(),
        "d1://synthetic".to_string(),
        synthetic_chain_link("audit://p16-21/d1"),
        PINNED_NOW_UNIX_MS,
    )
    .expect("constructs (the register check happens at record time)");
    let err = register
        .record_enablement(enablement)
        .expect_err("an unknown jurisdiction refuses");
    assert!(matches!(err, RegisterError::UnknownJurisdiction { .. }));
    // Register the fixture row, then a duplicate enablement refuses.
    register
        .register_jurisdiction(
            JurisdictionEntry::new(
                "fixture-iso".to_string(),
                "Fixture ISO (synthetic)".to_string(),
                "fixture participation model".to_string(),
                MarketPosture::Ferc2222,
                synthetic_timeline(),
            )
            .expect("valid jurisdiction row"),
        )
        .expect("registers once");
    let first = TenantMarketEnablement::enabled_by_d1(
        SYNTHETIC_TENANT.to_string(),
        "fixture-iso".to_string(),
        "d1://synthetic".to_string(),
        synthetic_chain_link("audit://p16-21/d1"),
        PINNED_NOW_UNIX_MS,
    )
    .expect("valid");
    register.record_enablement(first).expect("records once");
    let second = TenantMarketEnablement::enabled_by_d1(
        SYNTHETIC_TENANT.to_string(),
        "fixture-iso".to_string(),
        "d1://synthetic-2".to_string(),
        synthetic_chain_link("audit://p16-21/d1-2"),
        PINNED_NOW_UNIX_MS,
    )
    .expect("valid");
    let err = register
        .record_enablement(second)
        .expect_err("a duplicate enablement refuses");
    assert!(matches!(err, RegisterError::DuplicateIdentity { .. }));
}
