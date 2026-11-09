// P2.31 — The as-of (point-in-time) replay engine: re-deriving a past
//         decision's input snapshot from the versioned record history and
//         comparing it with what the decision actually used (ledger P2.31
//         step 5; foundations §6.4).
//
// This is DISTINCT from the P2.24 simulator replay (`replay` subcommand):
// P2.24 re-runs the SCENARIO; this module re-derives the DATA VIEW a past
// decision saw — "which version of each record was current at the moment
// the decision was made".
//
// As-of semantics (the same rule the Postgres adapter implements, D-C4):
// per storage identity `(asset_id, event_time, quantity, source_sequence)`,
// the latest version whose correction timestamp is <= the as-of instant;
// the original version (no correction timestamp) is ALWAYS visible; the
// default (no as-of) returns the latest version.
//
// Comparison semantics are the P2.24 ones unchanged: the comparison runs on
// the frozen field set `(asset_id, event_time, quantity, value, quality,
// source_sequence)` via [`crate::compare::compare`] — never on
// `ingest_time` (P2.13 §3). A correction's metadata (corrected_at, actor,
// reason) is VIEW-SELECTION input only; it never enters the comparison.
//
// Data source: RECORDS FILES, not the database. The recorded sim-runner
// dependency surface is deliberately free of database/broker infrastructure
// (`apps/sim-runner/Cargo.toml`), so the versions file is an export of the
// stored telemetry in the canonical record shape plus the correction
// metadata columns (the runbook, `25_exit_gate_runbook.md`, carries the SQL
// export that produces it). The selection function is pure and identical in
// shape to the adapter's SQL predicate, so a future TelemetryReader-wired
// variant maps onto it without a redesign (the recorded P2.24 follow-up
// pattern).
//
// Exit-code contract (the recorded P2.24 contract, applied to this
// entrypoint): pass → 0, mismatch → 1, usage/config error → 2.
//
// Everything this module handles is synthetic, simulation-only telemetry
// (the entire 0.x line): no field read, no field write, no control
// authority, no safety or certification implication.

use serde::{Deserialize, Serialize};

use crate::compare::{self, Comparison, RecordDiff};
use crate::replay::{records_from_json, ReplayError, ReplayRecord};
use crate::report::{exit_code_for_verdict, VERDICT_FAIL, VERDICT_PASS};

// --- The versioned record -------------------------------------------------------

/// One stored VERSION of a telemetry record: the canonical replay record
/// (the P2.24 comparison field set, no `ingest_time`) plus the correction
/// metadata the as-of view selection needs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VersionedRecord {
    /// The canonical record fields, flattened (the comparison field set).
    #[serde(flatten)]
    pub record: ReplayRecord,
    /// The correction timestamp, Unix milliseconds. Absent exactly on the
    /// ORIGINAL version (version 1) — which is always visible in an as-of
    /// view.
    #[serde(default)]
    pub corrected_at_unix_ms: Option<i64>,
    /// The correction actor (diagnostic metadata; not compared).
    #[serde(default)]
    pub corrected_by: Option<String>,
    /// The correction reason (diagnostic metadata; not compared).
    #[serde(default)]
    pub correction_reason: Option<String>,
}

/// The storage identity of one record's versions (the P2.04 §8 / P2.09
/// identity plus the quantity grain).
fn identity(record: &ReplayRecord) -> (&str, i64, &str, u64) {
    (
        record.asset_id.as_str(),
        record.event_time_unix_ms,
        record.quantity.as_str(),
        record.source_sequence,
    )
}

// --- The as-of view selection (pure) ---------------------------------------------

/// Derives the as-of view (P2.31 step 3 semantics, pure): per identity, the
/// latest version visible at `as_of_unix_ms`; `None` = the latest version
/// overall. The original version (`corrected_at_unix_ms = None`) is always
/// visible, so an as-of view never un-sees that a record existed — it only
/// decides WHICH version was current. Where several versions of one identity
/// are visible, the one with the greatest correction timestamp wins; the
/// input order is otherwise irrelevant (the selection is order-independent).
pub fn derive_as_of_snapshot(
    versions: &[VersionedRecord],
    as_of_unix_ms: Option<i64>,
) -> Vec<ReplayRecord> {
    use std::collections::BTreeMap;
    // identity → (ordering key, record). The ordering key is the correction
    // timestamp with the original sorted lowest (i64::MIN).
    let mut selected: BTreeMap<(&str, i64, &str, u64), (i64, &ReplayRecord)> = BTreeMap::new();
    for versioned in versions {
        let corrected = versioned.corrected_at_unix_ms.unwrap_or(i64::MIN);
        if let Some(as_of) = as_of_unix_ms {
            // The original (None) is always visible; corrections only from
            // their corrected_at onward.
            if versioned.corrected_at_unix_ms.is_some() && corrected > as_of {
                continue;
            }
        }
        match selected.entry(identity(&versioned.record)) {
            std::collections::btree_map::Entry::Vacant(slot) => {
                slot.insert((corrected, &versioned.record));
            }
            std::collections::btree_map::Entry::Occupied(mut slot) => {
                if corrected >= slot.get().0 {
                    slot.insert((corrected, &versioned.record));
                }
            }
        }
    }
    selected.into_values().map(|(_, record)| record.clone()).collect()
}

// --- The report -------------------------------------------------------------------

/// The as-of replay report (P2.31 step 5): the as-of instant, the record
/// counts, and the match/mismatch list WITH per-record diffs (the same diff
/// type the P2.24 report carries). Serialized with stable field names.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AsOfReplayReport {
    /// The as-of instant the snapshot was derived at (Unix ms); `null` =
    /// latest version (no as-of named).
    pub as_of_unix_ms: Option<i64>,
    /// Versions in the versioned history file.
    pub versions_count: usize,
    /// Records in the re-derived as-of snapshot (what the decision SHOULD
    /// have seen).
    pub snapshot_count: usize,
    /// Records in the decision-input file (what the decision ACTUALLY used).
    pub decision_input_count: usize,
    /// Records identical on the frozen comparison tuple.
    pub match_count: usize,
    /// Number of per-record differences.
    pub mismatch_count: usize,
    /// `"pass"` or `"fail"` — the gate asserts this, not the exit code.
    pub verdict: String,
    /// The mismatch list with per-record diffs (empty on a pass).
    pub mismatches: Vec<RecordDiff>,
}

impl AsOfReplayReport {
    /// Builds the report from the as-of instant, the versioned-history size,
    /// and the comparison outcome (reference = the decision input, candidate
    /// = the re-derived as-of snapshot).
    pub fn build(as_of_unix_ms: Option<i64>, versions_count: usize, comparison: Comparison) -> Self {
        let verdict = if comparison.is_pass() {
            VERDICT_PASS
        } else {
            VERDICT_FAIL
        };
        Self {
            as_of_unix_ms,
            versions_count,
            snapshot_count: comparison.candidate_count,
            decision_input_count: comparison.reference_count,
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
    /// ten rendered diffs).
    pub fn human_summary(&self) -> String {
        let mut out = String::new();
        match self.as_of_unix_ms {
            Some(as_of) => out.push_str(&format!(
                "as-of replay {} — as of {as_of} ms\n",
                self.verdict.to_uppercase()
            )),
            None => out.push_str(&format!(
                "as-of replay {} — latest version (no as-of named)\n",
                self.verdict.to_uppercase()
            )),
        }
        out.push_str(&format!(
            "records: decision input {}, as-of snapshot {}, matched {}, mismatched {}\n",
            self.decision_input_count, self.snapshot_count, self.match_count, self.mismatch_count
        ));
        if self.mismatches.is_empty() {
            out.push_str("no mismatches\n");
            return out;
        }
        out.push_str(&format!(
            "first {} of {} mismatches:\n",
            self.mismatches.len().min(10),
            self.mismatches.len()
        ));
        for diff in self.mismatches.iter().take(10) {
            out.push_str(&format!(
                "  [{field}] asset {asset} @ {time} ms, {quantity}\n",
                field = match diff.field {
                    crate::compare::DiffField::Value => "value",
                    crate::compare::DiffField::Quality => "quality",
                    crate::compare::DiffField::MissingInCandidate => "missing in snapshot",
                    crate::compare::DiffField::MissingInReference => "missing in decision input",
                },
                asset = diff.asset_id,
                time = diff.event_time_unix_ms,
                quantity = diff.quantity,
            ));
        }
        out
    }
}

// --- The run -----------------------------------------------------------------------

/// Reads a versioned-history file (the JSON array of [`VersionedRecord`]).
pub fn read_versions_file(path: &str) -> Result<Vec<VersionedRecord>, ReplayError> {
    let text = std::fs::read_to_string(path).map_err(|e| {
        ReplayError::Io(format!("could not read the versions file '{path}': {e}"))
    })?;
    serde_json::from_str(&text).map_err(|e| {
        ReplayError::Config(format!(
            "the versions file is not a versioned-records array: {e}"
        ))
    })
}

/// Runs the as-of replay (P2.31 step 5): derive the decision's input
/// snapshot from the versioned history at `as_of_unix_ms` (`None` = latest
/// version) and compare it against the decision-input records file on the
/// frozen P2.24 comparison tuple.
pub fn run_as_of(
    versions_path: &str,
    as_of_unix_ms: Option<i64>,
    decision_input_path: &str,
) -> Result<AsOfReplayReport, ReplayError> {
    let versions = read_versions_file(versions_path)?;
    let snapshot = derive_as_of_snapshot(&versions, as_of_unix_ms);
    // The decision-input file is the P2.24 records-file form (the canonical
    // comparison field set).
    let decision_input_text = std::fs::read_to_string(decision_input_path).map_err(|e| {
        ReplayError::Io(format!(
            "could not read the decision-input file '{decision_input_path}': {e}"
        ))
    })?;
    let decision_input = records_from_json(&decision_input_text)?;
    // An empty decision input is a legitimate comparison (the decision may
    // legitimately have seen zero records) — no special-casing.
    let comparison = compare::compare(&decision_input, &snapshot);
    Ok(AsOfReplayReport::build(as_of_unix_ms, versions.len(), comparison))
}

/// The exit code for an as-of replay verdict (the recorded P2.24 contract:
/// pass 0, mismatch 1, anything else 2).
pub fn exit_code(report: &AsOfReplayReport) -> i32 {
    exit_code_for_verdict(&report.verdict)
}

// --- A small self-check -------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::replay::ReplayValue;

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

    fn versioned(record: ReplayRecord, corrected_at: Option<i64>) -> VersionedRecord {
        VersionedRecord {
            record,
            corrected_at_unix_ms: corrected_at,
            corrected_by: corrected_at.map(|_| "operator:test".to_string()),
            correction_reason: corrected_at.map(|_| "test".to_string()),
        }
    }

    #[test]
    fn the_original_is_always_visible_and_the_default_is_the_latest_version() {
        let versions = vec![
            versioned(record(1, 1.0), None),
            versioned(record(1, 2.0), Some(1_000)),
        ];
        // No as-of → latest version.
        assert_eq!(derive_as_of_snapshot(&versions, None), vec![record(1, 2.0)]);
        // Any as-of, even before everything, still shows the ORIGINAL.
        assert_eq!(
            derive_as_of_snapshot(&versions, Some(0)),
            vec![record(1, 1.0)]
        );
    }

    #[test]
    fn the_as_of_boundary_is_inclusive_on_the_correction_timestamp() {
        let versions = vec![
            versioned(record(1, 1.0), None),
            versioned(record(1, 2.0), Some(1_000)),
        ];
        // corrected_at <= as_of is visible (the ledger: "correction
        // timestamp <= the as-of time").
        assert_eq!(
            derive_as_of_snapshot(&versions, Some(1_000)),
            vec![record(1, 2.0)]
        );
        assert_eq!(
            derive_as_of_snapshot(&versions, Some(999)),
            vec![record(1, 1.0)]
        );
    }

    #[test]
    fn the_selection_is_order_independent() {
        let a = vec![
            versioned(record(1, 1.0), None),
            versioned(record(1, 2.0), Some(1_000)),
            versioned(record(1, 3.0), Some(2_000)),
        ];
        let b = a.iter().rev().cloned().collect::<Vec<_>>();
        assert_eq!(
            derive_as_of_snapshot(&a, Some(1_500)),
            derive_as_of_snapshot(&b, Some(1_500))
        );
        assert_eq!(
            derive_as_of_snapshot(&a, Some(1_500)),
            vec![record(1, 2.0)]
        );
    }
}
