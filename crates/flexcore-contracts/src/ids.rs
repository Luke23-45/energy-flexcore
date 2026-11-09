// P1.05 — Identifier representation: `FlexUuid` over `[u8; 16]`
//
// Part of the flexcore-contracts P1.05 deliverable. FlexCore identifiers are
// RFC 9562 UUID-shaped 128-bit values (RESEARCH.md §6: PostgreSQL 18 offers
// built-in `uuidv7()`; the storage layer decides UUIDv4 vs UUIDv7 per
// ledger P1.12). This contract type deliberately provides **representation
// only**: parse (canonical hyphenated form), format, nil, and byte access.
//
// ID *generation* (random v4 or time-ordered v7) requires entropy and a
// version policy; that is an application-layer concern behind the
// id-generation port defined in P1.07. No `uuid` crate dependency is taken —
// this is a std-only implementation, and the dependency-policy check
// (tools/check_dependency_rule.py) would reject any external addition here
// anyway.
//
// Parse accepts the canonical hyphenated form (`8-4-4-4-12` hexadecimal
// groups) in lower or upper case and always formats lower case. Version and
// variant bits are not enforced here: the version policy belongs to the
// generation port, not to the wire representation.

/// Errors returned when parsing a [`FlexUuid`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlexUuidError {
    /// The input is not exactly 36 characters (32 hex digits plus 4 hyphens).
    InvalidLength,
    /// A character that must be a hexadecimal digit is not one.
    InvalidCharacter,
    /// A character at a hyphen position (8, 13, 18, 23) is not a hyphen.
    InvalidHyphenation,
}

impl std::fmt::Display for FlexUuidError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FlexUuidError::InvalidLength => {
                write!(f, "FlexUuid must be 36 characters in canonical hyphenated form")
            }
            FlexUuidError::InvalidCharacter => {
                write!(f, "FlexUuid hex groups may contain 0-9, a-f, and A-F only")
            }
            FlexUuidError::InvalidHyphenation => {
                write!(f, "FlexUuid requires hyphens at positions 8, 13, 18, and 23")
            }
        }
    }
}

impl std::error::Error for FlexUuidError {}

/// A 128-bit FlexCore identifier, carried as `[u8; 16]` in big-endian order.
///
/// Representation only — see the module header for the generation rule. The
/// typed identifier wrappers (`TenantId`, `SiteId`, `AssetId`, …) live in the
/// domain crate (P1.06 preparation) and wrap this type so a bare UUID can
/// never be confused between entity kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize)]
#[serde(into = "String", try_from = "String")]
pub struct FlexUuid([u8; 16]);

impl FlexUuid {
    /// The nil UUID (all zero bytes).
    pub const NIL: FlexUuid = FlexUuid([0u8; 16]);

    /// The nil UUID (all zero bytes).
    pub const fn nil() -> Self {
        Self::NIL
    }

    /// Constructs from the 16 bytes, in big-endian order.
    pub const fn from_bytes(bytes: [u8; 16]) -> Self {
        Self(bytes)
    }

    /// The 16 bytes, in big-endian order.
    pub const fn to_bytes(self) -> [u8; 16] {
        self.0
    }

    /// A borrowed view of the 16 bytes.
    pub const fn as_bytes(&self) -> &[u8; 16] {
        &self.0
    }

    /// Parses the canonical hyphenated form (lower or upper case hex).
    pub fn parse(s: &str) -> Result<Self, FlexUuidError> {
        let bytes = s.as_bytes();
        if bytes.len() != 36 {
            return Err(FlexUuidError::InvalidLength);
        }
        let mut out = [0u8; 16];
        let mut byte_index = 0usize;
        let mut high: u8 = 0;
        let mut have_high = false;
        for (i, &b) in bytes.iter().enumerate() {
            if matches!(i, 8 | 13 | 18 | 23) {
                if b != b'-' {
                    return Err(FlexUuidError::InvalidHyphenation);
                }
                continue;
            }
            let v = hex_value(b).ok_or(FlexUuidError::InvalidCharacter)?;
            if have_high {
                out[byte_index] = (high << 4) | v;
                byte_index += 1;
                have_high = false;
            } else {
                high = v;
                have_high = true;
            }
        }
        Ok(Self(out))
    }

    /// Formats the canonical hyphenated lower-case form.
    pub fn to_hyphenated(self) -> String {
        let mut s = String::with_capacity(36);
        for (i, b) in self.0.iter().enumerate() {
            if matches!(i, 4 | 6 | 8 | 10) {
                s.push('-');
            }
            s.push(hex_digit(b >> 4));
            s.push(hex_digit(b & 0x0f));
        }
        s
    }
}

fn hex_value(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

fn hex_digit(n: u8) -> char {
    match n {
        0..=9 => (b'0' + n) as char,
        _ => (b'a' + (n - 10)) as char,
    }
}

impl std::fmt::Display for FlexUuid {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.to_hyphenated())
    }
}

impl std::str::FromStr for FlexUuid {
    type Err = FlexUuidError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

impl TryFrom<String> for FlexUuid {
    type Error = FlexUuidError;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        Self::parse(&s)
    }
}

impl From<FlexUuid> for String {
    fn from(v: FlexUuid) -> Self {
        v.to_hyphenated()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "0192a5e0-8c3d-7e2f-9a1b-3c4d5e6f7a8b";

    #[test]
    fn nil_is_all_zero_and_formats_canonically() {
        let nil = FlexUuid::nil();
        assert!(nil.as_bytes().iter().all(|&b| b == 0));
        assert_eq!(nil.to_string(), "00000000-0000-0000-0000-000000000000");
        assert_eq!(FlexUuid::parse("00000000-0000-0000-0000-000000000000").unwrap(), nil);
    }

    #[test]
    fn from_bytes_and_to_bytes_round_trip() {
        let bytes = [
            0x01, 0x92, 0xa5, 0xe0, 0x8c, 0x3d, 0x7e, 0x2f, 0x9a, 0x1b, 0x3c, 0x4d, 0x5e, 0x6f,
            0x7a, 0x8b,
        ];
        let id = FlexUuid::from_bytes(bytes);
        assert_eq!(id.to_bytes(), bytes);
        assert_eq!(id.as_bytes(), &bytes);
    }

    #[test]
    fn parse_and_format_round_trip() {
        let id = FlexUuid::parse(SAMPLE).unwrap();
        assert_eq!(id.to_hyphenated(), SAMPLE);
        assert_eq!(id.to_string(), SAMPLE);
        // Upper-case input is accepted and normalised to lower-case output.
        let upper = FlexUuid::parse(&SAMPLE.to_uppercase()).unwrap();
        assert_eq!(upper, id);
        assert_eq!(upper.to_hyphenated(), SAMPLE);
        // FromStr and TryFrom<String> agree with parse.
        let via_str: FlexUuid = SAMPLE.parse().unwrap();
        assert_eq!(via_str, id);
        let via_try_from = FlexUuid::try_from(SAMPLE.to_string()).unwrap();
        assert_eq!(via_try_from, id);
    }

    #[test]
    fn parse_rejects_malformed_input() {
        // Too short / too long.
        assert_eq!(FlexUuid::parse(""), Err(FlexUuidError::InvalidLength));
        assert_eq!(FlexUuid::parse("0192a5e08c3d7e2f9a1b3c4d5e6f7a8b"), Err(FlexUuidError::InvalidLength));
        assert_eq!(
            FlexUuid::parse("0192a5e0-8c3d-7e2f-9a1b-3c4d5e6f7a8bX"),
            Err(FlexUuidError::InvalidLength)
        );
        // Bad character.
        assert_eq!(
            FlexUuid::parse("0192a5g0-8c3d-7e2f-9a1b-3c4d5e6f7a8b"),
            Err(FlexUuidError::InvalidCharacter)
        );
        // Misplaced hyphens.
        assert_eq!(
            FlexUuid::parse("0192a5e0-8c3d-7e2f-9a1b3c4d5e6f7a8b-"),
            Err(FlexUuidError::InvalidHyphenation)
        );
        assert_eq!(
            FlexUuid::parse("0192a5e0_8c3d-7e2f-9a1b-3c4d5e6f7a8b"),
            Err(FlexUuidError::InvalidHyphenation)
        );
    }

    #[test]
    fn serde_round_trips_as_the_hyphenated_string() {
        let id = FlexUuid::parse(SAMPLE).unwrap();
        let json = serde_json::to_value(&id).unwrap();
        assert_eq!(json, serde_json::json!(SAMPLE));
        let back: FlexUuid = serde_json::from_value(json).unwrap();
        assert_eq!(back, id);
        // The nil form round trips too.
        let nil = FlexUuid::nil();
        let back: FlexUuid =
            serde_json::from_value(serde_json::to_value(&nil).unwrap()).unwrap();
        assert_eq!(back, nil);
    }

    #[test]
    fn serde_rejects_malformed_strings() {
        assert!(serde_json::from_value::<FlexUuid>(serde_json::json!("not-a-uuid")).is_err());
    }
}
