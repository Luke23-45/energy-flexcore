// P1.13 — FlexUuid ↔ PostgreSQL uuid mapping.
//
// RECORDED DECISION (crate README §"identifier mapping"): identifiers travel
// as canonical hyphenated TEXT — bound as text parameters and cast with
// `::uuid` inside the SQL statement, selected back with `::text`. The
// workspace sqlx pin does not enable the `uuid` feature, and keeping the
// conversion here means the contracts `FlexUuid` ([u8; 16]) stays the single
// representation of record; the database never sees anything but a
// canonical UUID value because the cast rejects anything else.

use flexcore_contracts::ids::FlexUuid;

/// The canonical hyphenated text form of an identifier (what gets bound as
/// a text parameter and cast with `::uuid`).
pub(crate) fn uuid_text(id: FlexUuid) -> String {
    id.to_hyphenated()
}

/// Parses an identifier read back from a `::text` selection.
pub(crate) fn uuid_from_text(raw: &str) -> Result<FlexUuid, String> {
    FlexUuid::parse(raw)
}

#[cfg(test)]
mod tests {
    use super::*;
    use flexcore_contracts::ids::FlexUuid;

    const SAMPLE: &str = "0192a5e0-8c3d-7e2f-9a1b-3c4d5e6f7a8b";

    #[test]
    fn text_round_trip_is_exact_and_lowercase() {
        let id = FlexUuid::parse(SAMPLE).expect("valid sample uuid");
        let text = uuid_text(id);
        assert_eq!(text, SAMPLE);
        assert_eq!(uuid_from_text(&text).expect("round trip"), id);
    }

    #[test]
    fn nil_uuid_maps_to_the_canonical_nil_text() {
        assert_eq!(uuid_text(FlexUuid::NIL), "00000000-0000-0000-0000-000000000000");
    }

    #[test]
    fn malformed_text_is_rejected_as_a_decode_failure() {
        assert!(uuid_from_text("not-a-uuid").is_err());
        assert!(uuid_from_text("").is_err());
        // The decode failure carried alongside such a rejection names the
        // column (a schema identifier, never raw storage text) and uses
        // the stable decode code.
        let backend = crate::error::decode_failure("tenants.tenant_id");
        assert_eq!(backend.code(), "E_STORE_DECODE");
        assert!(backend.message().contains("tenants.tenant_id"));
    }
}
