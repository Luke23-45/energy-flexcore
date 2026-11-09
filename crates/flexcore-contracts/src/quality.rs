// P0.08 — Telemetry quality vocabulary: six states, exactly one per value (Phase 0 freeze)
// P1.05 — flexcore-contracts implementation (moved from the Phase 0 seed in
//         flexcore-domain and upgraded with serde).
//
// Implements `ledger/phase-00-product-freeze/08_telemetry_quality_vocabulary.md`.
// Normative source: report §10.4; foundations §6.4 governs on conflict.
//
// Frozen rules encoded here:
// - Every telemetry value carries **exactly one** canonical quality state;
//   there is no seventh state anywhere in the Phase 0 freeze.
// - Richer source states (for example `SUBSTITUTED`, `OUT_OF_RANGE`,
//   `UNSYNCHRONIZED`, `COMMUNICATION_LOSS`, `UNRELIABLE`) are **mapped to one
//   of the six canonical states at the adapter boundary**, with the original
//   source code retained as diagnostic metadata. The canonical vocabulary
//   does not grow; source detail survives as metadata.
// - **Non-silencing rule:** a quality flag must not be silently converted
//   into a normal reading. Downgrading, dropping, or omitting a quality flag
//   at any boundary is prohibited. Enforced here by construction:
//   [`TelemetryQuality::Good`] has no conversion from any source state — no
//   function in this module produces `GOOD` from a mapped source state.
// - Estimation rule carried forward from foundations §6.4 (Phase 4
//   carry-forward): estimation/interpolation is explicit, tagged, bounded by
//   policy, and disallowed for safety-critical authorization unless approved.
// - Per-use-case acceptability: every use case defines which quality states
//   are acceptable; there is no universal acceptability set (not modelled in
//   this module).
//
// Serialization note: both enums serialize as their SCREAMING_SNAKE_CASE
// canonical names (for example `GOOD`, `OUT_OF_RANGE`) so the wire vocabulary
// matches the frozen vocabulary documents.

use serde::{Deserialize, Serialize};

/// The six canonical telemetry quality states (report §10.4; exactly one per
/// value).
///
/// Frozen vocabulary, with the §10.4 meanings:
/// - `GOOD` — valid, fresh, within range, and from a trusted source.
/// - `SUSPECT` — received and parsed, but a plausibility, range, or
///   consistency check failed.
/// - `BAD` — the source reports the value invalid, or a hard validation rule
///   failed.
/// - `MISSING` — no value was received for the expected interval.
/// - `ESTIMATED` — derived, interpolated, substituted, or model-generated
///   rather than measured.
/// - `STALE` — last known value whose age exceeds the use-case freshness
///   limit.
///
/// There is no seventh state; no adapter, simulator, or plugin may introduce
/// an additional canonical quality state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TelemetryQuality {
    /// `GOOD` — valid, fresh, within range, and from a trusted source.
    Good,
    /// `SUSPECT` — received and parsed, but a plausibility, range, or
    /// consistency check failed.
    Suspect,
    /// `BAD` — the source reports the value invalid, or a hard validation
    /// rule failed.
    Bad,
    /// `MISSING` — no value was received for the expected interval.
    Missing,
    /// `ESTIMATED` — derived, interpolated, substituted, or model-generated
    /// rather than measured.
    Estimated,
    /// `STALE` — last known value whose age exceeds the use-case freshness
    /// limit.
    Stale,
}

impl TelemetryQuality {
    /// All six canonical states, in frozen vocabulary order.
    pub const ALL: [TelemetryQuality; 6] = [
        TelemetryQuality::Good,
        TelemetryQuality::Suspect,
        TelemetryQuality::Bad,
        TelemetryQuality::Missing,
        TelemetryQuality::Estimated,
        TelemetryQuality::Stale,
    ];

    /// The canonical SCREAMING_SNAKE name from the frozen vocabulary.
    pub const fn name(self) -> &'static str {
        match self {
            TelemetryQuality::Good => "GOOD",
            TelemetryQuality::Suspect => "SUSPECT",
            TelemetryQuality::Bad => "BAD",
            TelemetryQuality::Missing => "MISSING",
            TelemetryQuality::Estimated => "ESTIMATED",
            TelemetryQuality::Stale => "STALE",
        }
    }
}

/// Richer source quality states that may arrive from an adapter or vendor
/// source (report §10.4).
///
/// These are the §10.4 example source states. They are **not** canonical
/// states: each must be mapped to one of the six [`TelemetryQuality`] states
/// at the adapter boundary via [`map_source_state`], and the original source
/// code retained as diagnostic metadata. The canonical vocabulary never grows
/// to include these.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SourceQualityState {
    /// `SUBSTITUTED` — the value was substituted rather than measured
    /// (report §10.4 source-state example).
    Substituted,
    /// `OUT_OF_RANGE` — the value failed a range check at the source
    /// (report §10.4 source-state example).
    OutOfRange,
    /// `UNSYNCHRONIZED` — the source clock was not synchronized, so the
    /// timestamp/freshness cannot be trusted (report §10.4 source-state
    /// example).
    Unsynchronized,
    /// `COMMUNICATION_LOSS` — communication with the source was lost; at best
    /// a last known value exists (report §10.4 source-state example).
    CommunicationLoss,
    /// `UNRELIABLE` — the source is marked unreliable (report §10.4
    /// source-state example).
    Unreliable,
}

impl SourceQualityState {
    /// All source states this module models (the §10.4 examples).
    pub const ALL: [SourceQualityState; 5] = [
        SourceQualityState::Substituted,
        SourceQualityState::OutOfRange,
        SourceQualityState::Unsynchronized,
        SourceQualityState::CommunicationLoss,
        SourceQualityState::Unreliable,
    ];
}

/// Maps a [`SourceQualityState`] to exactly one of the six canonical
/// [`TelemetryQuality`] states (report §10.4 mapping rule).
///
/// The frozen document fixes the *rule* (every source state maps to one of
/// the six, original code retained as metadata) and gives meanings per
/// canonical state; the per-source-state choices below follow those meanings.
/// They are the module's declared mapping (recorded in the Phase 0 HANDOVER
/// assumptions) and are covered by tests:
///
/// | Source state | Canonical state | Rationale (§10.4 meaning) |
/// |---|---|---|
/// | `SUBSTITUTED` | `ESTIMATED` | `ESTIMATED` explicitly covers "substituted" values |
/// | `OUT_OF_RANGE` | `SUSPECT` | `SUSPECT` covers failed range/plausibility checks |
/// | `UNSYNCHRONIZED` | `SUSPECT` | a time-consistency check failed on a received, parsed value |
/// | `COMMUNICATION_LOSS` | `STALE` | only a last known value exists, whose age grows past freshness |
/// | `UNRELIABLE` | `SUSPECT` | received and parsed, but source trust/consistency is in question |
///
/// Non-silencing by construction: no arm of this mapping returns
/// [`TelemetryQuality::Good`], and no `From`/`Into` conversion exists from
/// any [`SourceQualityState`] to `GOOD`. The caller retains the original
/// source state as diagnostic metadata; this function never erases it.
pub fn map_source_state(s: SourceQualityState) -> TelemetryQuality {
    match s {
        SourceQualityState::Substituted => TelemetryQuality::Estimated,
        SourceQualityState::OutOfRange => TelemetryQuality::Suspect,
        SourceQualityState::Unsynchronized => TelemetryQuality::Suspect,
        SourceQualityState::CommunicationLoss => TelemetryQuality::Stale,
        SourceQualityState::Unreliable => TelemetryQuality::Suspect,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exactly_six_canonical_states() {
        assert_eq!(TelemetryQuality::ALL.len(), 6);
    }

    #[test]
    fn six_variant_exhaustive_match() {
        // P1.05 acceptance: a match over the quality enum must be exhaustive
        // across exactly the six frozen variants — adding a seventh variant
        // breaks this match.
        for q in TelemetryQuality::ALL {
            let canonical_name = match q {
                TelemetryQuality::Good => "GOOD",
                TelemetryQuality::Suspect => "SUSPECT",
                TelemetryQuality::Bad => "BAD",
                TelemetryQuality::Missing => "MISSING",
                TelemetryQuality::Estimated => "ESTIMATED",
                TelemetryQuality::Stale => "STALE",
            };
            assert_eq!(canonical_name, q.name());
        }
    }

    #[test]
    fn canonical_names_match_frozen_vocabulary() {
        let names: [&str; 6] = [
            TelemetryQuality::Good.name(),
            TelemetryQuality::Suspect.name(),
            TelemetryQuality::Bad.name(),
            TelemetryQuality::Missing.name(),
            TelemetryQuality::Estimated.name(),
            TelemetryQuality::Stale.name(),
        ];
        assert_eq!(
            names,
            ["GOOD", "SUSPECT", "BAD", "MISSING", "ESTIMATED", "STALE"]
        );
    }

    #[test]
    fn quality_serde_uses_the_frozen_names() {
        for q in TelemetryQuality::ALL {
            let json = serde_json::to_value(&q).unwrap();
            assert_eq!(json, serde_json::json!(q.name()));
            let back: TelemetryQuality = serde_json::from_value(json).unwrap();
            assert_eq!(back, q);
        }
    }

    #[test]
    fn source_state_serde_uses_the_frozen_names() {
        for s in SourceQualityState::ALL {
            let json = serde_json::to_value(&s).unwrap();
            let back: SourceQualityState = serde_json::from_value(json).unwrap();
            assert_eq!(back, s);
        }
    }

    #[test]
    fn quality_serde_rejects_unknown_names() {
        // No seventh state may deserialize, and no invented name passes.
        assert!(serde_json::from_value::<TelemetryQuality>(serde_json::json!("UNKNOWN")).is_err());
        assert!(serde_json::from_value::<TelemetryQuality>(serde_json::json!("good")).is_err());
    }

    #[test]
    fn every_source_state_is_mapped() {
        for s in SourceQualityState::ALL {
            let mapped = map_source_state(s);
            assert!(
                TelemetryQuality::ALL.contains(&mapped),
                "mapping must always yield one of the six canonical states"
            );
        }
    }

    #[test]
    fn mapping_covers_each_source_state_variant() {
        // This asserts the expected mapping for each source-state variant
        // explicitly (three of them share SUSPECT, per the documented table
        // and the Phase 0 HANDOVER assumptions).
        assert_eq!(
            map_source_state(SourceQualityState::Substituted),
            TelemetryQuality::Estimated
        );
        assert_eq!(
            map_source_state(SourceQualityState::OutOfRange),
            TelemetryQuality::Suspect
        );
        assert_eq!(
            map_source_state(SourceQualityState::Unsynchronized),
            TelemetryQuality::Suspect
        );
        assert_eq!(
            map_source_state(SourceQualityState::CommunicationLoss),
            TelemetryQuality::Stale
        );
        assert_eq!(
            map_source_state(SourceQualityState::Unreliable),
            TelemetryQuality::Suspect
        );
    }

    #[test]
    fn non_silencing_no_source_state_maps_to_good() {
        for s in SourceQualityState::ALL {
            assert_ne!(
                map_source_state(s),
                TelemetryQuality::Good,
                "a source state must never be silently mapped to GOOD"
            );
        }
    }

    #[test]
    fn non_silencing_good_is_never_constructed_from_a_source_state() {
        // GOOD appears in this module only as the canonical enum variant and
        // in TelemetryQuality::ALL; it is deliberately not produced by
        // map_source_state. There is also no From<SourceQualityState> for
        // TelemetryQuality — assert the mapping result set excludes GOOD.
        for s in SourceQualityState::ALL {
            let mapped = map_source_state(s);
            assert!(
                matches!(
                    mapped,
                    TelemetryQuality::Suspect
                        | TelemetryQuality::Bad
                        | TelemetryQuality::Missing
                        | TelemetryQuality::Estimated
                        | TelemetryQuality::Stale
                ),
                "mapped quality must be a non-GOOD canonical state"
            );
        }
    }
}
