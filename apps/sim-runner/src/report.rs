// P2.24 — The replay report: the recorded outcome of one replay comparison.
//
// The report carries everything the ledger P2.24 step 4 requires — scenario
// version, seed, code version, record counts, match count, the mismatch
// list WITH per-record diffs, and a pass/fail verdict — in two renderings:
// JSON (machine-readable; the CI P2.27 scenario-regression gate asserts the
// VERDICT, not just the exit code) and a human-readable summary.
//
// Exit-code contract (P2.24 step 4, recorded once):
//
// | outcome                    | exit code |
// |----------------------------|-----------|
// | replay pass (zero mismatches) | 0      |
// | replay mismatch (verdict "fail") | 1   |
// | usage / configuration error | 2        |
//
// The mapping is a pure function ([`exit_code_for_verdict`]) so the
// contract is testable without running the binary.

use serde::Serialize;

use crate::compare::{Comparison, RecordDiff};

/// Exit code for a passing replay.
pub const EXIT_PASS: i32 = 0;
/// Exit code for a replay that found mismatches.
pub const EXIT_MISMATCH: i32 = 1;
/// Exit code for a usage or configuration error.
pub const EXIT_USAGE: i32 = 2;

/// The verdict vocabulary of the report (asserted by the P2.27 gate).
pub const VERDICT_PASS: &str = "pass";
pub const VERDICT_FAIL: &str = "fail";

/// Maps a verdict onto the recorded exit code. An unknown verdict is a
/// usage error (2), never a silent pass — a verdict spelling bug must not
/// be able to green a CI gate.
pub fn exit_code_for_verdict(verdict: &str) -> i32 {
    match verdict {
        VERDICT_PASS => EXIT_PASS,
        VERDICT_FAIL => EXIT_MISMATCH,
        _ => EXIT_USAGE,
    }
}

/// The replay report (P2.24 step 4). Serialized with stable field names —
/// the P2.27 scenario-regression gate reads these fields.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ReplayReport {
    /// The scenario id (from the validated scenario artefact).
    pub scenario_id: String,
    /// The scenario version — replay always names it (P2.13 §4).
    pub scenario_version: String,
    /// The master seed in effect for the candidate run (D-SC4).
    pub seed: u64,
    /// The code version the candidate run names (`--code-version` or the
    /// `FLEXCORE_CODE_VERSION` environment; a generation change breaks
    /// replay honestly, P2.13 §4).
    pub code_version: String,
    /// Records in the reference set.
    pub reference_count: usize,
    /// Records in the candidate (fresh run) set.
    pub candidate_count: usize,
    /// Records identical on the frozen comparison tuple.
    pub match_count: usize,
    /// Number of per-record differences.
    pub mismatch_count: usize,
    /// `"pass"` or `"fail"` — the gate asserts this, not the exit code.
    pub verdict: String,
    /// The mismatch list with per-record diffs (empty on a pass).
    pub mismatches: Vec<RecordDiff>,
}

impl ReplayReport {
    /// Builds the report from the run identity and the comparison outcome.
    pub fn build(
        scenario_id: &str,
        scenario_version: &str,
        seed: u64,
        code_version: &str,
        comparison: Comparison,
    ) -> Self {
        let verdict = if comparison.is_pass() {
            VERDICT_PASS
        } else {
            VERDICT_FAIL
        };
        Self {
            scenario_id: scenario_id.to_string(),
            scenario_version: scenario_version.to_string(),
            seed,
            code_version: code_version.to_string(),
            reference_count: comparison.reference_count,
            candidate_count: comparison.candidate_count,
            match_count: comparison.match_count,
            mismatch_count: comparison.mismatches.len(),
            verdict: verdict.to_string(),
            mismatches: comparison.mismatches,
        }
    }

    /// The JSON rendering (pretty; stable field order for reviewable diffs).
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// The human-readable summary (one verdict line, the counts, and up to
    /// [`SUMMARY_MISMATCH_LIMIT`] rendered diffs).
    pub fn human_summary(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!(
            "replay {} — scenario '{}:{}', seed {}, code version {}\n",
            self.verdict.to_uppercase(),
            self.scenario_id,
            self.scenario_version,
            self.seed,
            self.code_version
        ));
        out.push_str(&format!(
            "records: reference {}, candidate {}, matched {}, mismatched {}\n",
            self.reference_count, self.candidate_count, self.match_count, self.mismatch_count
        ));
        if self.mismatches.is_empty() {
            out.push_str("no mismatches\n");
            return out;
        }
        out.push_str(&format!(
            "first {} of {} mismatches:\n",
            self.mismatches.len().min(SUMMARY_MISMATCH_LIMIT),
            self.mismatches.len()
        ));
        for diff in self.mismatches.iter().take(SUMMARY_MISMATCH_LIMIT) {
            out.push_str(&format!(
                "  [{field}] asset {asset} @ {time} ms, {quantity}\n",
                field = match diff.field {
                    crate::compare::DiffField::Value => "value",
                    crate::compare::DiffField::Quality => "quality",
                    crate::compare::DiffField::MissingInCandidate => "missing in candidate",
                    crate::compare::DiffField::MissingInReference => "missing in reference",
                },
                asset = diff.asset_id,
                time = diff.event_time_unix_ms,
                quantity = diff.quantity,
            ));
            if let Some(reference) = &diff.reference {
                out.push_str(&format!("    reference: {}\n", diff_line(reference)));
            }
            if let Some(candidate) = &diff.candidate {
                out.push_str(&format!("    candidate: {}\n", diff_line(candidate)));
            }
        }
        out
    }
}

/// How many diffs the human summary renders in full (the report JSON always
/// carries every mismatch).
const SUMMARY_MISMATCH_LIMIT: usize = 20;

/// One-line rendering of a record inside a diff.
fn diff_line(record: &crate::replay::ReplayRecord) -> String {
    let value = match &record.value {
        crate::replay::ReplayValue::Power { mw } => format!("{mw} MW"),
        crate::replay::ReplayValue::Reactive { mvar, convention } => {
            format!("{mvar} MVAr ({convention})")
        }
        crate::replay::ReplayValue::Fraction { value } => format!("{value} (fraction)"),
        crate::replay::ReplayValue::Energy { mwh } => format!("{mwh} MWh"),
        crate::replay::ReplayValue::Flag { value } => format!("{value} (flag)"),
        crate::replay::ReplayValue::State { value } => format!("\"{value}\" (state)"),
    };
    format!(
        "seq {} = {value}, quality {}",
        record.source_sequence, record.quality
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compare::DiffField;
    use crate::replay::{ReplayRecord, ReplayValue};

    fn record(seq: u64, mw: f64) -> ReplayRecord {
        ReplayRecord {
            asset_id: "0192a5e0-0000-7e2f-9a1b-3c4d5e6f7a8c".to_string(),
            event_time_unix_ms: 1_780_272_000_000,
            quantity: "active_power".to_string(),
            value: ReplayValue::Power { mw },
            quality: "GOOD".to_string(),
            source_sequence: seq,
        }
    }

    fn sample_report(pass: bool) -> ReplayReport {
        let reference = vec![record(1, 1.0), record(2, 2.0)];
        let candidate = if pass {
            reference.clone()
        } else {
            vec![record(1, 1.0), record(2, 9.0)]
        };
        let comparison = crate::compare::compare(&reference, &candidate);
        ReplayReport::build("synthetic-portfolio-exit-gate", "v1", 20260601, "test-code", comparison)
    }

    #[test]
    fn the_report_carries_every_required_field() {
        let report = sample_report(false);
        assert_eq!(report.scenario_id, "synthetic-portfolio-exit-gate");
        assert_eq!(report.scenario_version, "v1");
        assert_eq!(report.seed, 20260601);
        assert_eq!(report.code_version, "test-code");
        assert_eq!(report.reference_count, 2);
        assert_eq!(report.candidate_count, 2);
        assert_eq!(report.match_count, 1);
        assert_eq!(report.mismatch_count, 1);
        assert_eq!(report.verdict, VERDICT_FAIL);
        assert_eq!(report.mismatches.len(), 1);
        assert_eq!(report.mismatches[0].field, DiffField::Value);
    }

    #[test]
    fn the_json_rendering_holds_the_required_field_names() {
        let text = sample_report(false).to_json().unwrap();
        let value: serde_json::Value = serde_json::from_str(&text).unwrap();
        for field in [
            "scenario_id",
            "scenario_version",
            "seed",
            "code_version",
            "reference_count",
            "candidate_count",
            "match_count",
            "mismatch_count",
            "verdict",
            "mismatches",
        ] {
            assert!(value.get(field).is_some(), "required field {field} missing");
        }
        assert_eq!(value["verdict"], "fail");
        assert_eq!(value["mismatches"][0]["field"], "value");
        assert!(value["mismatches"][0]["reference"].is_object());
        assert!(value["mismatches"][0]["candidate"].is_object());
    }

    #[test]
    fn the_human_summary_states_the_verdict_and_counts() {
        let pass = sample_report(true).human_summary();
        assert!(pass.contains("PASS"));
        assert!(pass.contains("no mismatches"));
        assert!(pass.contains("v1"));
        assert!(pass.contains("20260601"));
        let fail = sample_report(false).human_summary();
        assert!(fail.contains("FAIL"));
        assert!(fail.contains("mismatched 1"));
        assert!(fail.contains("reference:"));
        assert!(fail.contains("candidate:"));
    }

    #[test]
    fn the_exit_code_contract_is_pure_and_fail_closed() {
        assert_eq!(exit_code_for_verdict(VERDICT_PASS), EXIT_PASS);
        assert_eq!(exit_code_for_verdict(VERDICT_PASS), 0);
        assert_eq!(exit_code_for_verdict(VERDICT_FAIL), EXIT_MISMATCH);
        assert_eq!(exit_code_for_verdict(VERDICT_FAIL), 1);
        assert_eq!(exit_code_for_verdict("garbage"), EXIT_USAGE);
        assert_eq!(exit_code_for_verdict("garbage"), 2);
    }
}
