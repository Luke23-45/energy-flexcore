// P1.07 — The clock port: deterministic time for tests (P1.07 step 2).
//
// The contracts crate (P0.09) makes `event_time` (when something occurred)
// and `ingest_time` (when FlexCore received it) **distinct types with no
// conversion between them** — merging or substituting them is a compile
// error. The clock port honours that rule structurally: it offers two
// methods, one per timestamp type, both derived from the same underlying
// instant by the implementation. A single `now()` returning one type would
// silently invite the other call site to reuse it.
//
// Recorded decision (crate README): two methods instead of one `now()`.
// The fake clock returns the same instant for both within a tick, which is
// the correct P0.09 relationship (`ingest_time >= event_time`).

use flexcore_contracts::time_model::{EventTime, IngestTime};

/// The time source the application and adapters use for wall-clock
/// timestamps. Implementations derive `now_event_time` and
/// `now_ingest_time` from the same underlying instant; the two return
/// types are the distinct P0.09 contracts types and are never merged.
///
/// Test determinism: production implementations wrap the platform clock;
/// the in-crate fake ([`crate::fakes::FixedClock`]) returns a fixed,
/// advanceable instant so application tests are fully deterministic.
pub trait Clock {
    /// The current instant as the contracts `EventTime` (UTC enforced by
    /// the type's constructor).
    fn now_event_time(&self) -> EventTime;

    /// The current instant as the contracts `IngestTime` (UTC enforced by
    /// the type's constructor). The underlying instant is the same one
    /// `now_event_time` reports.
    fn now_ingest_time(&self) -> IngestTime;
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A minimal inline implementation proving the trait is implementable
    /// over one instant and that both methods return the distinct types.
    struct InstantClock;

    impl Clock for InstantClock {
        fn now_event_time(&self) -> EventTime {
            EventTime::from_unix_millis(1_000).expect("1_000 ms is a valid UTC timestamp")
        }
        fn now_ingest_time(&self) -> IngestTime {
            IngestTime::from_unix_millis(1_000).expect("1_000 ms is a valid UTC timestamp")
        }
    }

    #[test]
    fn both_time_views_come_from_one_instant() {
        let clock = InstantClock;
        assert_eq!(clock.now_event_time().unix_millis(), 1_000);
        assert_eq!(clock.now_ingest_time().unix_millis(), 1_000);
        // The types stay distinct (P0.09): no conversion exists between
        // them — merging them is a compile error, asserted here only by
        // the fact that the two methods have distinct return types.
    }
}
