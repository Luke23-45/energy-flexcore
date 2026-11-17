// P15.04 — The API-token HUMAN GATE boundary.
//
// The registered API token for the Transparency Platform REST API is a
// HUMAN GATE input (program conventions §5): it is UNSIGNED until a human
// registers it, and this module carries the REFERENCE, never the
// material. There is no field a token could hide in — the closed field
// vocabulary makes inline material structurally impossible (the P7.05
// secret-reference rule and the P8.32 signer-key-reference rule, applied
// to the price adapter). A rejected reference's VALUE is never echoed
// back (it may be the token itself).
//
// The gate state is RECORDED, not asserted: the adapter ships with the
// gate UNSIGNED, and no request is ever sent in this program (conventions
// §1 loop bans; the first live call is a human/lab gate after P15-E-2
// closes and a human registers the token).
//
// Control status: a credentials-boundary record. Nothing here authorizes,
// issues, dispatches, or bids.

use std::fmt;

/// The accepted token-reference schemes (the P7.05 secret-reference
/// schemes; the deployment key store resolves them — the adapter never
/// resolves material itself).
pub const TOKEN_REFERENCE_SCHEMES: [&str; 3] = ["env:", "file:", "secret-store:"];

/// The recorded gate state (UNSIGNED — a HUMAN GATE input; pinned by
/// tests).
pub const TOKEN_GATE_STATEMENT: &str = "the Transparency Platform API token is a HUMAN GATE \
     input: UNSIGNED until a human registers it (program conventions 5). The adapter \
     carries scheme-prefixed token REFERENCES only; token material is structurally \
     impossible (the closed field vocabulary has no field a key could hide in); a \
     rejected reference's value is never echoed back. No request is sent in this program \
     (the first live call is a human/lab gate after P15-E-2 closes).";

/// The gate states (closed): the adapter ships UNSIGNED; only the human
/// registration act moves it (and that act happens in the deployment
/// configuration, never in this code).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TokenGateState {
    /// No token registered: the adapter refuses every live request (the
    /// fail-closed default — the absence of credentials can never look
    /// like success).
    Unsigned,
    /// A human registered the token; the adapter holds a scheme-prefixed
    /// REFERENCE to the deployment key store's entry.
    RegisteredByHuman,
}

impl TokenGateState {
    /// The canonical spelling (records).
    pub const fn as_str(self) -> &'static str {
        match self {
            TokenGateState::Unsigned => "UNSIGNED",
            TokenGateState::RegisteredByHuman => "REGISTERED-BY-HUMAN",
        }
    }
}

/// Why a token reference was refused. The error never echoes the rejected
/// VALUE (it may be inline material).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenReferenceError {
    /// The reference did not use a scheme prefix (likely inline material).
    InlineMaterialRefused,
    /// The reference was empty after the scheme prefix.
    EmptyReference,
}

impl TokenReferenceError {
    /// The stable machine-readable error name.
    pub fn name(&self) -> &'static str {
        match self {
            TokenReferenceError::InlineMaterialRefused => "A44_TOKEN_INLINE_MATERIAL_REFUSED",
            TokenReferenceError::EmptyReference => "A44_TOKEN_REFERENCE_EMPTY",
        }
    }
}

impl fmt::Display for TokenReferenceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            // Deliberately does NOT echo the value.
            TokenReferenceError::InlineMaterialRefused => write!(
                f,
                "the token reference must use one of the env:, file:, or secret-store: \
                 schemes and name a location; the adapter carries token REFERENCES only \
                 and can never carry token material"
            ),
            TokenReferenceError::EmptyReference => {
                write!(f, "the token reference names an empty location")
            }
        }
    }
}

impl std::error::Error for TokenReferenceError {}

/// Validates a token reference (fail-closed; the VALUE is never echoed
/// back in any error).
pub fn validate_token_reference(reference: &str) -> Result<(), TokenReferenceError> {
    for scheme in TOKEN_REFERENCE_SCHEMES {
        if let Some(rest) = reference.strip_prefix(scheme) {
            if rest.trim().is_empty() {
                return Err(TokenReferenceError::EmptyReference);
            }
            return Ok(());
        }
    }
    Err(TokenReferenceError::InlineMaterialRefused)
}

// ---------------------------------------------------------------------------
// Tests (WRITTEN-UNRUN — no cargo command was executed in this loop; the
// human build/test gate owns the run. Every expected value below is
// hand-derived from the rules above. Nothing is fabricated.)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_gate_ships_unsigned() {
        assert_eq!(TokenGateState::Unsigned.as_str(), "UNSIGNED");
        assert_eq!(TokenGateState::RegisteredByHuman.as_str(), "REGISTERED-BY-HUMAN");
        assert!(TOKEN_GATE_STATEMENT.contains("UNSIGNED until a human registers"));
        assert!(TOKEN_GATE_STATEMENT.contains("structurally impossible"));
    }

    #[test]
    fn scheme_prefixed_references_validate() {
        for reference in [
            "env:ENTSOE_SECURITY_TOKEN",
            "file:///run/secrets/entsoe-token",
            "secret-store:market-data/entsoe/token",
        ] {
            validate_token_reference(reference)
                .expect("a scheme-prefixed reference validates");
        }
    }

    #[test]
    fn inline_material_is_refused_and_never_echoed() {
        let inline = "RAW-TOKEN-0123456789abcdef";
        let err = validate_token_reference(inline).expect_err("inline refused");
        assert_eq!(err.name(), "A44_TOKEN_INLINE_MATERIAL_REFUSED");
        let text = format!("{err}");
        assert!(
            !text.contains("RAW-TOKEN"),
            "a rejected token reference VALUE is never echoed back (it may be the token)"
        );
    }

    #[test]
    fn an_empty_location_is_refused() {
        let err = validate_token_reference("env:").expect_err("empty location refused");
        assert_eq!(err.name(), "A44_TOKEN_REFERENCE_EMPTY");
    }
}
