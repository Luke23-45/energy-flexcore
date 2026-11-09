// P2.16 step 3 — The asset availability rule (pure, no I/O), DECIDED ONCE.
//
// Ledger P2.16 step 3: "Publish `AssetAvailabilityChanged` when an asset's
// availability changes (from the asset's telemetry-derived availability —
// define the availability rule once: e.g. availability derived from the
// latest quality + connectivity signals, recorded in the design)."
//
// Inputs (per `ledger/phase-02-asset-telemetry/04_telemetry_model.md`: the
// availability inputs are the asset's `availability` quantity — a boolean
// signal, unit `1` — plus the record's quality and the source's
// connectivity):
//
// - `latest_quality`: the canonical quality state (P0.08) of the asset's
//   latest `availability` record, or `None` when no availability record has
//   ever been received;
// - `latest_value`: the boolean value of that record (`None` when the
//   record carries no value — e.g. a MISSING gap marker has none by
//   construction, P2.04 §3);
// - `source_reporting`: the connectivity signal — whether the telemetry
//   source is currently reporting for the asset (stage-1 health in the
//   ingestion pipeline; a provider whose health is `Unavailable` reports
//   nothing, so no re-assessment ever runs from its frames — the signal is
//   `true` at every pipeline emission point and exists so the rule stays
//   complete and unit-testable).
//
// THE RULE (P2.16, decided once; the `SUSPECT`-tolerant boundary follows
// the P2.08 hard-vs-soft distinction — a SUSPECT value is well-formed and
// interpretable, so it may still drive the flag):
//
// - no signal at all (`latest_quality == None`) → **Unknown**;
// - `MISSING` (gap marker, no value) → **Unchanged** — a missing interval
//   says nothing new about availability; the previous assessment stands
//   (unknown/unchanged per the ledger);
// - `STALE` (age-based, event-time age vs the use-case freshness limit) →
//   **Unchanged** with the reason recorded — the previous availability
//   stands until a fresh signal arrives (staleness never invents a state,
//   P2.08 producer separation);
// - `BAD` (hard validation failure) → **Unknown** — the flag cannot be
//   trusted (non-silencing: never read a flag that failed hard validation);
// - `GOOD`/`SUSPECT`:
//   - value `true` and the source is reporting → **Available**;
//   - value `true` but the source is NOT reporting → **Unknown** (the
//     connectivity signal contradicts a stale "available" claim);
//   - value `false` → **Unavailable** (the asset reports itself
//     unavailable — its own word outranks the connectivity signal);
//   - value absent → **Unknown**.
//
// Change detection ([`availability_changed`]): the event fires exactly when
// the previous assessment state differs from the new one; an `Unchanged`
// result resolves to the previous state (it can never produce an event by
// itself). The first concrete assessment (Unknown → Available, say) IS a
// change and emits.
//
// The concrete availability value is NOT carried by the frozen
// `AssetAvailabilityChanged` payload (P1.09 shape; recorded assumption in
// the flexcore-events README): the event is the fact "availability
// changed"; consumers re-derive the concrete state from the asset's
// `availability` telemetry.

use flexcore_contracts::quality::TelemetryQuality;

/// The computed availability of an asset (P2.16 step 3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AvailabilityState {
    /// The asset reports itself available on a trustworthy signal.
    Available,
    /// The asset reports itself unavailable on a trustworthy signal.
    Unavailable,
    /// No trustworthy signal exists right now (never received, hard-failed,
    /// or contradicted by the connectivity signal).
    Unknown,
    /// The latest signal says nothing new (MISSING marker or STALE): the
    /// caller keeps its previously recorded availability. Resolved to the
    /// previous state before change detection — it never differs from it.
    Unchanged,
}

/// One availability assessment: the state plus the recorded reason (the
/// audit-facing evidence, in the spirit of the P0.08 §4 flag-plus-cause
/// rule).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AvailabilityAssessment {
    pub state: AvailabilityState,
    pub reason: &'static str,
}

/// The inputs of [`availability`] (see the module header for the full
/// contract).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AvailabilitySignalInput {
    /// The quality of the asset's latest `availability` record.
    pub latest_quality: Option<TelemetryQuality>,
    /// The boolean value of that record (`None` when it carries no value).
    pub latest_value: Option<bool>,
    /// The connectivity signal: is the telemetry source currently
    /// reporting for this asset?
    pub source_reporting: bool,
}

/// The availability rule (P2.16 step 3, DECIDED ONCE — see the module
/// header for the full decision table). Pure: no clock read, no I/O — the
/// staleness decision is upstream (the record's canonical quality already
/// carries STALE when the freshness gate marked it).
pub fn availability(input: &AvailabilitySignalInput) -> AvailabilityAssessment {
    match input.latest_quality {
        None => AvailabilityAssessment {
            state: AvailabilityState::Unknown,
            reason: "no availability signal has ever been received for the asset",
        },
        Some(TelemetryQuality::Missing) => AvailabilityAssessment {
            state: AvailabilityState::Unchanged,
            reason: "the latest interval is a MISSING gap marker; the previous \
                     availability stands",
        },
        Some(TelemetryQuality::Stale) => AvailabilityAssessment {
            state: AvailabilityState::Unchanged,
            reason: "the latest availability signal is STALE; the previous \
                     availability stands until a fresh signal arrives",
        },
        Some(TelemetryQuality::Bad) => AvailabilityAssessment {
            state: AvailabilityState::Unknown,
            reason: "the latest availability signal failed hard validation; \
                     the flag cannot be trusted",
        },
        Some(TelemetryQuality::Good | TelemetryQuality::Suspect) => {
            match (input.latest_value, input.source_reporting) {
                (Some(true), true) => AvailabilityAssessment {
                    state: AvailabilityState::Available,
                    reason: "the asset reports available on a \
                             GOOD/SUSPECT signal and the source is reporting",
                },
                (Some(true), false) => AvailabilityAssessment {
                    state: AvailabilityState::Unknown,
                    reason: "the availability signal reports available but the \
                             source connectivity is down; the claim cannot be \
                             confirmed",
                },
                (Some(false), _) => AvailabilityAssessment {
                    state: AvailabilityState::Unavailable,
                    reason: "the asset reports itself unavailable on a \
                             GOOD/SUSPECT signal",
                },
                (None, _) => AvailabilityAssessment {
                    state: AvailabilityState::Unknown,
                    reason: "the availability record carries no value",
                },
            }
        }
    }
}

/// Whether an availability change must be published (P2.16 step 3): the
/// event fires exactly when the previous assessment state differs from the
/// new one. An `Unchanged` result resolves to the previous state, so it can
/// never produce a change by itself.
pub fn availability_changed(previous: AvailabilityState, current: AvailabilityState) -> bool {
    let resolved_current = match current {
        AvailabilityState::Unchanged => previous,
        other => other,
    };
    previous != resolved_current
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(q: Option<TelemetryQuality>, v: Option<bool>, reporting: bool) -> AvailabilitySignalInput {
        AvailabilitySignalInput {
            latest_quality: q,
            latest_value: v,
            source_reporting: reporting,
        }
    }

    #[test]
    fn no_signal_is_unknown() {
        let a = availability(&input(None, None, true));
        assert_eq!(a.state, AvailabilityState::Unknown);
        assert!(!a.reason.is_empty());
    }

    #[test]
    fn good_true_with_reporting_is_available() {
        let a = availability(&input(Some(TelemetryQuality::Good), Some(true), true));
        assert_eq!(a.state, AvailabilityState::Available);
        // SUSPECT is well-formed and interpretable — it may still drive the
        // flag (the P2.08 hard-vs-soft boundary).
        let s = availability(&input(Some(TelemetryQuality::Suspect), Some(true), true));
        assert_eq!(s.state, AvailabilityState::Available);
    }

    #[test]
    fn good_true_without_reporting_is_unknown() {
        let a = availability(&input(Some(TelemetryQuality::Good), Some(true), false));
        assert_eq!(a.state, AvailabilityState::Unknown);
        assert!(a.reason.contains("connectivity"));
    }

    #[test]
    fn false_value_is_unavailable_even_when_the_source_is_down() {
        // The asset's own word outranks the connectivity signal.
        for reporting in [true, false] {
            let a = availability(&input(Some(TelemetryQuality::Good), Some(false), reporting));
            assert_eq!(a.state, AvailabilityState::Unavailable, "reporting={reporting}");
        }
    }

    #[test]
    fn stale_and_missing_signals_leave_the_availability_unchanged() {
        // STALE → unchanged WITH the reason recorded (the ledger's rule).
        let stale = availability(&input(Some(TelemetryQuality::Stale), Some(true), true));
        assert_eq!(stale.state, AvailabilityState::Unchanged);
        assert!(stale.reason.contains("STALE"));
        // MISSING → unchanged (unknown/unchanged).
        let missing = availability(&input(Some(TelemetryQuality::Missing), None, true));
        assert_eq!(missing.state, AvailabilityState::Unchanged);
        // Unchanged can never produce a change event.
        for previous in [
            AvailabilityState::Available,
            AvailabilityState::Unavailable,
            AvailabilityState::Unknown,
        ] {
            assert!(!availability_changed(previous, AvailabilityState::Unchanged));
        }
    }

    #[test]
    fn bad_signal_is_unknown_never_trusted() {
        let a = availability(&input(Some(TelemetryQuality::Bad), Some(true), true));
        assert_eq!(a.state, AvailabilityState::Unknown);
        assert!(a.reason.contains("hard validation"));
    }

    #[test]
    fn valueless_good_signal_is_unknown() {
        let a = availability(&input(Some(TelemetryQuality::Good), None, true));
        assert_eq!(a.state, AvailabilityState::Unknown);
    }

    #[test]
    fn change_is_exactly_a_state_difference() {
        // First concrete assessment: Unknown -> Available is a change.
        assert!(availability_changed(AvailabilityState::Unknown, AvailabilityState::Available));
        // A real flip.
        assert!(availability_changed(
            AvailabilityState::Available,
            AvailabilityState::Unavailable
        ));
        assert!(availability_changed(
            AvailabilityState::Unavailable,
            AvailabilityState::Available
        ));
        // Same state never changes.
        assert!(!availability_changed(
            AvailabilityState::Available,
            AvailabilityState::Available
        ));
        assert!(!availability_changed(
            AvailabilityState::Unknown,
            AvailabilityState::Unknown
        ));
        // Unknown -> Unavailable is a change (we learned something).
        assert!(availability_changed(
            AvailabilityState::Unknown,
            AvailabilityState::Unavailable
        ));
    }
}
