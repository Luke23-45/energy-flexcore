// P14.02 §2.1 — DERControl timing windows (decision record
// 02_p14_02_aggregator_server.md §2.1).
//
// A DERControl carries an interval (start, duration) plus optional
// randomization offsets [S-1][S-3] (UNVERIFIED exact semantics — see the
// register). This module models the window as DECLARED data and provides
// the pure scheduling questions the platform answers about it:
//
// - [`DerControlWindow::active_at`] — is the control active at instant `t`?
// - [`DerControlWindow::effective_start`] — the start instant after the
//   declared randomization offset.
// - [`DerControlWindow::overlaps`] — do two windows overlap in time? (The
//   primacy resolution in `addressing.rs` needs this to know when two
//   controls compete at all.)
//
// The randomization offsets are carried verbatim and the effective start
// ADDS the declared offset to the declared start (the conservative
// reading); the standard's own randomization semantics (per-device jitter,
// distribution) are UNVERIFIED and must be confirmed at implementation
// time before any wire-facing scheduling is trusted. Nothing here commands
// a device: these are scheduling QUESTIONS, answered for proposals and
// tests only.

use serde::{Deserialize, Serialize};

use crate::model::TimeType;

/// The timing window of a DERControl (P14.02 §2.1). Constructed from the
/// resource's declared fields ([`crate::model::DERControl::window`]) —
/// never inferred.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct DerControlWindow {
    /// The declared interval start (TimeType seconds).
    pub start: TimeType,
    /// The declared interval duration in seconds.
    pub duration_seconds: u32,
    /// The declared randomized-start offset in seconds, when carried
    /// (UNVERIFIED semantics; carried and applied as DECLARED data).
    pub randomized_start: Option<u32>,
    /// The declared randomized-duration offset in seconds, when carried
    /// (UNVERIFIED semantics; recorded, not applied).
    pub randomized_duration: Option<u32>,
}

impl DerControlWindow {
    /// The effective start instant: the declared start plus the declared
    /// randomization offset (the conservative reading; UNVERIFIED — module
    /// header). Overflow saturates rather than wrapping.
    pub fn effective_start(&self) -> TimeType {
        let offset = self.randomized_start.unwrap_or(0) as i64;
        TimeType(self.start.0.saturating_add(offset))
    }

    /// The end instant of the window (start + duration, computed from the
    /// EFFECTIVE start; saturating).
    pub fn end(&self) -> TimeType {
        TimeType(
            self.effective_start()
                .0
                .saturating_add(self.duration_seconds as i64),
        )
    }

    /// Whether the control is active at instant `t`: `Some(true)` within
    /// `[effective_start, end)`, `Some(false)` outside, and `None` only if
    /// the window carries no duration (a start-only control declares no
    /// interval — the caller decides; the module never guesses).
    pub fn active_at(&self, t: TimeType) -> Option<bool> {
        if self.duration_seconds == 0 && self.randomized_duration.is_none() {
            return None;
        }
        let start = self.effective_start();
        Some(t.0 >= start.0 && t.0 < self.end().0)
    }

    /// Whether this window overlaps another in time (half-open intervals:
    /// touching endpoints do not overlap).
    pub fn overlaps(&self, other: &DerControlWindow) -> bool {
        self.effective_start().0 < other.end().0 && other.effective_start().0 < self.end().0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn window(start: i64, duration: u32) -> DerControlWindow {
        DerControlWindow {
            start: TimeType(start),
            duration_seconds: duration,
            randomized_start: None,
            randomized_duration: None,
        }
    }

    #[test]
    fn active_at_is_half_open() {
        let w = window(100, 300);
        assert_eq!(w.active_at(TimeType(99)), Some(false));
        assert_eq!(w.active_at(TimeType(100)), Some(true));
        assert_eq!(w.active_at(TimeType(399)), Some(true));
        // The end instant is exclusive.
        assert_eq!(w.active_at(TimeType(400)), Some(false));
        // A start-only window answers None: no interval declared.
        let start_only = DerControlWindow {
            start: TimeType(100),
            duration_seconds: 0,
            randomized_start: None,
            randomized_duration: None,
        };
        assert_eq!(start_only.active_at(TimeType(150)), None);
    }

    #[test]
    fn randomization_shifts_the_effective_start_as_declared_data() {
        let w = DerControlWindow {
            start: TimeType(100),
            duration_seconds: 300,
            randomized_start: Some(50),
            randomized_duration: Some(10),
        };
        assert_eq!(w.effective_start(), TimeType(150));
        assert_eq!(w.end(), TimeType(450));
        assert_eq!(w.active_at(TimeType(120)), Some(false));
        assert_eq!(w.active_at(TimeType(160)), Some(true));
    }

    #[test]
    fn overlap_is_symmetric_and_exclusive_of_touching() {
        let a = window(100, 300);
        let b = window(200, 300);
        let c = window(400, 100);
        let touching = window(400, 0); // start == a.end()
        assert!(a.overlaps(&b));
        assert!(b.overlaps(&a));
        assert!(!a.overlaps(&c));
        // Touching endpoints (half-open) do not overlap.
        assert!(!a.overlaps(&touching));
    }

    #[test]
    fn saturating_arithmetic_never_wraps() {
        let w = DerControlWindow {
            start: TimeType(i64::MAX - 5),
            duration_seconds: 60,
            randomized_start: Some(120),
            randomized_duration: None,
        };
        assert_eq!(w.effective_start(), TimeType(i64::MAX));
        assert_eq!(w.end(), TimeType(i64::MAX));
    }
}
