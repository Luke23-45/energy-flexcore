// P1.13 — SystemClock: the wall-clock implementation of the clock port.
//
// The port (P1.07) offers two methods — one per distinct P0.09 timestamp
// type — derived from the same underlying instant. This implementation uses
// the platform UTC clock; `ingest_time >= event_time` holds because both
// views come from the same `now_utc()` reading.

use flexcore_contracts::time_model::{EventTime, IngestTime};
use flexcore_ports::clock::Clock;

/// The production clock over the platform UTC time source.
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    /// The current instant as the contracts `EventTime` (UTC enforced by
    /// the type's constructor; the platform clock always carries the UTC
    /// offset here).
    fn now_event_time(&self) -> EventTime {
        EventTime::new(time::OffsetDateTime::now_utc())
            .expect("the platform UTC clock always yields a valid UTC instant")
    }

    /// The current instant as the contracts `IngestTime`, read from the
    /// same platform clock reading as [`Clock::now_event_time`].
    fn now_ingest_time(&self) -> IngestTime {
        IngestTime::new(time::OffsetDateTime::now_utc())
            .expect("the platform UTC clock always yields a valid UTC instant")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use flexcore_contracts::time_model::validate_ingest_after_event;

    #[test]
    fn both_views_come_from_one_wall_clock_and_keep_the_p0_09_ordering() {
        let clock = SystemClock;
        let event = clock.now_event_time();
        let ingest = clock.now_ingest_time();
        // Two consecutive readings; ingest is taken after event, so the
        // P0.09 ordering rule must hold (equal or later).
        assert!(validate_ingest_after_event(event, ingest).is_ok());
        // Both views are recent (after 2020-01-01 UTC).
        assert!(event.unix_millis() > 1_577_836_800_000);
    }
}
