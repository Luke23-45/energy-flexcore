// P2.24 — The replay comparison: comparing two canonical record sets on the
// frozen comparison tuple, with a per-record diff.
//
// Rules this module encodes (ledger P2.24 step 3; P2.13 §3):
//
// - **The comparison fields are exactly** `(asset_id, event_time,
//   quantity, value, quality, source_sequence)`. The record type
//   ([`crate::replay::ReplayRecord`]) structurally cannot carry
//   `ingest_time`, so the never-compared rule holds by construction, not by
//   discipline.
// - **Identity** of a replay record for matching = `(asset_id, event_time,
//   quantity, source_sequence)` — the P2.04 §8 / P2.09 identity (the
//   quantity is part of it because one frame decodes to one record per
//   quantity under one shared sequence, P2.04 D-T2).
// - **A mismatch is reported with a per-record diff** naming WHICH field
//   diverged (value or quality) — the "fails honestly with a diff"
//   acceptance. Unmatched records (present on one side only) are reported
//   as `missing_in_candidate` / `missing_in_reference` rather than being
//   silently absorbed by a count-only check.
// - **Multiset semantics:** duplicate identities (the `duplicate` /
//   `conflicting_duplicate` fault kinds emit two frames under one
//   identity, P2.15) are matched pairwise in deterministic (sorted) order,
//   so two runs of the same faulted scenario compare equal and a run
//   against a de-duplicated reference shows the surplus explicitly.
// - **f64 equality is exact** (bit-identical determinism, P2.13 §3): same
//   scenario version + same seed + same code version must produce
//   identical values; anything else IS a replay break and must surface as
//   a mismatch, not be rounded into plausibility (P0.07: never round in a
//   way that changes meaning).

use serde::Serialize;

use crate::replay::ReplayRecord;

/// Which comparison field diverged for a mismatched pair.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DiffField {
    /// Same identity, different value.
    Value,
    /// Same identity, different quality state.
    Quality,
    /// The record exists only in the reference set.
    MissingInCandidate,
    /// The record exists only in the candidate set.
    MissingInReference,
}

/// One per-record difference. For a field mismatch both records travel (the
/// diff); for a one-sided record only the side that has it travels.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RecordDiff {
    pub field: DiffField,
    pub asset_id: String,
    pub event_time_unix_ms: i64,
    pub quantity: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reference: Option<ReplayRecord>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub candidate: Option<ReplayRecord>,
}

/// The outcome of one comparison.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Comparison {
    pub reference_count: usize,
    pub candidate_count: usize,
    pub match_count: usize,
    pub mismatches: Vec<RecordDiff>,
}

impl Comparison {
    /// Whether the two record sets are replay-identical.
    pub fn is_pass(&self) -> bool {
        self.mismatches.is_empty()
    }
}

/// The record's matching identity (the P2.04 §8 tuple plus the quantity).
fn identity(record: &ReplayRecord) -> (&str, i64, &str, u64) {
    (
        record.asset_id.as_str(),
        record.event_time_unix_ms,
        record.quantity.as_str(),
        record.source_sequence,
    )
}

/// Sorts records into the deterministic comparison order: identity, then
/// value/quality so identical identities pair up stably.
fn sorted(records: &[ReplayRecord]) -> Vec<&ReplayRecord> {
    let mut sorted: Vec<&ReplayRecord> = records.iter().collect();
    sorted.sort_by(|a, b| {
        identity(a)
            .cmp(&identity(b))
            .then_with(|| a.quality.cmp(&b.quality))
            .then_with(|| serde_json::to_string(&a.value).cmp(&serde_json::to_string(&b.value)))
    });
    sorted
}

/// Compares the reference record set against the candidate (fresh run) on
/// the frozen comparison tuple. Order-independent (both sides are sorted
/// first) — the pipeline tolerates out-of-order arrival (P2.09), so arrival
/// order is not a replay property.
pub fn compare(reference: &[ReplayRecord], candidate: &[ReplayRecord]) -> Comparison {
    let reference = sorted(reference);
    let candidate = sorted(candidate);
    let mut match_count = 0usize;
    let mut mismatches = Vec::new();
    let mut r = 0usize;
    let mut c = 0usize;
    while r < reference.len() || c < candidate.len() {
        match (reference.get(r), candidate.get(c)) {
            (Some(ref_rec), Some(cand_rec)) => match identity(ref_rec).cmp(&identity(cand_rec)) {
                std::cmp::Ordering::Less => {
                    mismatches.push(one_sided(DiffField::MissingInCandidate, ref_rec, true));
                    r += 1;
                }
                std::cmp::Ordering::Greater => {
                    mismatches.push(one_sided(DiffField::MissingInReference, cand_rec, false));
                    c += 1;
                }
                std::cmp::Ordering::Equal => {
                    if ref_rec.value != cand_rec.value {
                        mismatches.push(RecordDiff {
                            field: DiffField::Value,
                            asset_id: ref_rec.asset_id.clone(),
                            event_time_unix_ms: ref_rec.event_time_unix_ms,
                            quantity: ref_rec.quantity.clone(),
                            reference: Some((*ref_rec).clone()),
                            candidate: Some((*cand_rec).clone()),
                        });
                    } else if ref_rec.quality != cand_rec.quality {
                        mismatches.push(RecordDiff {
                            field: DiffField::Quality,
                            asset_id: ref_rec.asset_id.clone(),
                            event_time_unix_ms: ref_rec.event_time_unix_ms,
                            quantity: ref_rec.quantity.clone(),
                            reference: Some((*ref_rec).clone()),
                            candidate: Some((*cand_rec).clone()),
                        });
                    } else {
                        match_count += 1;
                    }
                    r += 1;
                    c += 1;
                }
            },
            (Some(ref_rec), None) => {
                mismatches.push(one_sided(DiffField::MissingInCandidate, ref_rec, true));
                r += 1;
            }
            (None, Some(cand_rec)) => {
                mismatches.push(one_sided(DiffField::MissingInReference, cand_rec, false));
                c += 1;
            }
            (None, None) => break,
        }
    }
    Comparison {
        reference_count: reference.len(),
        candidate_count: candidate.len(),
        match_count,
        mismatches,
    }
}

/// Builds a one-sided diff for a record that exists on only one side.
fn one_sided(field: DiffField, record: &ReplayRecord, from_reference: bool) -> RecordDiff {
    RecordDiff {
        field,
        asset_id: record.asset_id.clone(),
        event_time_unix_ms: record.event_time_unix_ms,
        quantity: record.quantity.clone(),
        reference: if from_reference {
            Some(record.clone())
        } else {
            None
        },
        candidate: if from_reference {
            None
        } else {
            Some(record.clone())
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::replay::ReplayValue;

    fn record(seq: u64, mw: f64, quality: &str) -> ReplayRecord {
        ReplayRecord {
            asset_id: "0192a5e0-0000-7e2f-9a1b-3c4d5e6f7a8c".to_string(),
            event_time_unix_ms: 1_780_272_000_000,
            quantity: "active_power".to_string(),
            value: ReplayValue::Power { mw },
            quality: quality.to_string(),
            source_sequence: seq,
        }
    }

    #[test]
    fn identical_sets_match_in_any_order() {
        let a = vec![record(1, 1.0, "GOOD"), record(2, 2.0, "GOOD")];
        let b = vec![record(2, 2.0, "GOOD"), record(1, 1.0, "GOOD")];
        let comparison = compare(&a, &b);
        assert!(comparison.is_pass());
        assert_eq!(comparison.match_count, 2);
        assert_eq!(comparison.mismatches, Vec::new());
    }

    #[test]
    fn a_value_difference_is_a_diff_naming_both_records() {
        let reference = vec![record(1, 1.0, "GOOD")];
        let candidate = vec![record(1, 7.5, "GOOD")];
        let comparison = compare(&reference, &candidate);
        assert!(!comparison.is_pass());
        assert_eq!(comparison.mismatches.len(), 1);
        assert_eq!(comparison.mismatches[0].field, DiffField::Value);
        assert_eq!(comparison.mismatches[0].reference.as_ref().unwrap().value, ReplayValue::Power { mw: 1.0 });
        assert_eq!(comparison.mismatches[0].candidate.as_ref().unwrap().value, ReplayValue::Power { mw: 7.5 });
    }

    #[test]
    fn a_quality_difference_is_its_own_field() {
        let comparison = compare(&[record(1, 1.0, "GOOD")], &[record(1, 1.0, "SUSPECT")]);
        assert_eq!(comparison.mismatches.len(), 1);
        assert_eq!(comparison.mismatches[0].field, DiffField::Quality);
    }

    #[test]
    fn one_sided_records_are_reported_not_absorbed() {
        let reference = vec![record(1, 1.0, "GOOD"), record(2, 2.0, "GOOD")];
        let candidate = vec![record(2, 2.0, "GOOD"), record(3, 3.0, "GOOD")];
        let comparison = compare(&reference, &candidate);
        assert_eq!(comparison.match_count, 1);
        assert_eq!(comparison.mismatches.len(), 2);
        assert!(comparison
            .mismatches
            .iter()
            .any(|d| d.field == DiffField::MissingInCandidate && d.source_sequence_of() == 1));
        assert!(comparison
            .mismatches
            .iter()
            .any(|d| d.field == DiffField::MissingInReference && d.source_sequence_of() == 3));
    }

    #[test]
    fn duplicate_identities_pair_up_as_multisets() {
        // Two runs of the same duplicate-faulted scenario: the doubled
        // identity appears twice on both sides and matches pairwise.
        let reference = vec![record(1, 1.0, "GOOD"), record(1, 1.0, "GOOD")];
        let candidate = vec![record(1, 1.0, "GOOD"), record(1, 1.0, "GOOD")];
        let comparison = compare(&reference, &candidate);
        assert!(comparison.is_pass());
        assert_eq!(comparison.match_count, 2);
        // One side de-duplicated: the surplus record is explicit.
        let comparison = compare(&reference, &[record(1, 1.0, "GOOD")]);
        assert_eq!(comparison.mismatches.len(), 1);
        assert_eq!(comparison.mismatches[0].field, DiffField::MissingInCandidate);
    }

    #[test]
    fn empty_sets_compare_pass_and_counts_are_faithful() {
        let comparison = compare(&[], &[]);
        assert!(comparison.is_pass());
        assert_eq!(comparison.reference_count, 0);
        assert_eq!(comparison.candidate_count, 0);
        let comparison = compare(&[record(1, 1.0, "GOOD")], &[]);
        assert_eq!(comparison.reference_count, 1);
        assert_eq!(comparison.candidate_count, 0);
        assert_eq!(comparison.mismatches.len(), 1);
    }

    impl RecordDiff {
        /// Test helper: the source_sequence of whichever side carries a record.
        fn source_sequence_of(&self) -> u64 {
            self.reference
                .as_ref()
                .or(self.candidate.as_ref())
                .expect("a diff always carries at least one record")
                .source_sequence
        }
    }
}
