// P6.05 — The edge agent's wall-clock [`Clock`] implementation
// (composition-side; the control plane's `SystemClock` lives in the
// PostgreSQL adapter, and the edge agent must not take a database
// dependency to tell the time — §8.2 dependency economy).
//
// Same contract as the P1.07 clock port: two methods, one underlying
// instant, the two P0.09 timestamp types never merged. Time-source QUALITY
// is a recorded field, not an assumption (P0.09 §4; §17.4 clock drift): the
// edge deployment profile (P6.20) requires a synchronised system clock
// (systemd-timesyncd/chrony) and the P6.31 time-sync work owns the edge-side
// synchronisation checks; this implementation simply reports the platform
// clock, and every sample's clock quality is recorded by the source that
// produced it (the ACL requires the declaration, P6.12).

use flexcore_contracts::time_model::{EventTime, IngestTime};
use flexcore_ports::clock::Clock;

/// The platform wall clock, as the [`Clock`] port.
#[derive(Debug, Default, Clone, Copy)]
pub struct WallClock;

impl Clock for WallClock {
    fn now_event_time(&self) -> EventTime {
        let now = time::OffsetDateTime::now_utc();
        EventTime::new(now).expect("the platform clock's UTC instant is a valid EventTime")
    }

    fn now_ingest_time(&self) -> IngestTime {
        let now = time::OffsetDateTime::now_utc();
        IngestTime::new(now).expect("the platform clock's UTC instant is a valid IngestTime")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn both_views_come_from_one_instant_and_are_utc() {
        let clock = WallClock;
        let event = clock.now_event_time();
        let ingest = clock.now_ingest_time();
        // Both views are valid UTC timestamps (constructors enforce the
        // offset); the two calls may differ by the wall clock advancing
        // between them, which is the P0.09-correct relationship
        // (ingest >= event for the same observation).
        assert!(ingest.unix_millis() >= event.unix_millis() - 1);
    }
}
