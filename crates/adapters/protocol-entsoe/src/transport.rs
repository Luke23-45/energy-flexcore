// P15.04 — The REST transport SEAM (unwired).
//
// The Transparency Platform REST API is an HTTPS GET API ([S-111]; the
// endpoint shape, parameter names, and response format are UNVERIFIED
// until checked against the live API guide). This module models the
// REQUEST (a typed query for one A44 day-ahead price document over one
// domain pairing and one period) and declares the transport port — with
// NO HTTP client, NO TLS stack, and NO socket: the default
// implementation answers EVERY request with the named `SeamNotWired`
// refusal. No request is ever sent in this program (conventions §1 loop
// bans; the first live call is a human/lab gate after P15-E-2 closes and
// a human registers the token).
//
// UNVERIFIED register (implementation-time checks against [S-111]):
//
// | # | Detail | Where |
// |---|---|---|
// | 1 | The endpoint URL shape | [`A44_QUERY_ENDPOINT`] |
// | 2 | The securityToken parameter name | [`A44_PARAM_SECURITY_TOKEN`] |
// | 3 | The documentType/in_Domain/out_Domain/periodStart/periodEnd parameter names | [`A44_PARAM_DOCUMENT_TYPE`] etc. |
// | 4 | The period-bound query format (YYYYMMDDHHMM, UTC) | `document::parse_period_bound` |
// | 5 | The response format (XML) and its encoding | `xml_seam` |
//
// The sync-trait rule of the port crate applies (no futures, no async
// runtime); the adapter decides how to bridge asynchrony on its own side.
//
// Control status: a designed, UNWIRED seam for READ-ONLY data ingestion.
// Nothing here authorizes, issues, dispatches, or bids; a price document
// is ingested DATA, never a command and never a bid input.

/// The endpoint path of the A44 query (UNVERIFIED per [S-111]; the
/// recorded shape is the platform's public REST endpoint).
pub const A44_QUERY_ENDPOINT: &str = "https://transparency.entsoe.eu/api";

/// The securityToken query-parameter name (UNVERIFIED per [S-111]; the
/// VALUE is supplied by the deployment at call time from the registered
/// token reference — never stored in this crate).
pub const A44_PARAM_SECURITY_TOKEN: &str = "securityToken";

/// The documentType query-parameter name (UNVERIFIED per [S-111]).
pub const A44_PARAM_DOCUMENT_TYPE: &str = "documentType";

/// The in_Domain query-parameter name (UNVERIFIED per [S-111]).
pub const A44_PARAM_IN_DOMAIN: &str = "in_Domain";

/// The out_Domain query-parameter name (UNVERIFIED per [S-111]).
pub const A44_PARAM_OUT_DOMAIN: &str = "out_Domain";

/// The periodStart query-parameter name (UNVERIFIED per [S-111]).
pub const A44_PARAM_PERIOD_START: &str = "periodStart";

/// The periodEnd query-parameter name (UNVERIFIED per [S-111]).
pub const A44_PARAM_PERIOD_END: &str = "periodEnd";

/// One A44 day-ahead price query: the domain pairing and the period.
/// The period bounds are the recorded YYYYMMDDHHMM strings (the query
/// format, UNVERIFIED); validation mirrors `document::parse_period_bound`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct A44PriceQuery {
    /// The in-domain area identifier (the platform's EIC-style area code
    /// as published; carried verbatim, UNVERIFIED vocabulary).
    pub in_domain: String,
    /// The out-domain area identifier.
    pub out_domain: String,
    /// The period start (YYYYMMDDHHMM, UTC; UNVERIFIED format).
    pub period_start: String,
    /// The period end (YYYYMMDDHHMM, UTC; UNVERIFIED format).
    pub period_end: String,
}

/// Why a query was refused. Named, matchable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum A44QueryError {
    /// An area identifier was empty or whitespace.
    EmptyArea,
    /// A period bound did not parse (delegates to the recorded format
    /// check).
    InvalidPeriodBound { value: String },
    /// The period was inverted (start must be before end).
    InvertedPeriod,
}

impl A44QueryError {
    /// The stable machine-readable error name.
    pub fn name(&self) -> &'static str {
        match self {
            A44QueryError::EmptyArea => "A44_QUERY_EMPTY_AREA",
            A44QueryError::InvalidPeriodBound { .. } => "A44_QUERY_INVALID_PERIOD_BOUND",
            A44QueryError::InvertedPeriod => "A44_QUERY_INVERTED_PERIOD",
        }
    }
}

impl std::fmt::Display for A44QueryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            A44QueryError::EmptyArea => {
                write!(f, "the in-domain and out-domain area identifiers must be non-empty")
            }
            A44QueryError::InvalidPeriodBound { value } => write!(
                f,
                "the period bound '{value}' does not parse as YYYYMMDDHHMM (UTC; \
                 UNVERIFIED per [S-111])"
            ),
            A44QueryError::InvertedPeriod => {
                write!(f, "the query period is inverted (start must be before end)")
            }
        }
    }
}

impl std::error::Error for A44QueryError {}

impl A44PriceQuery {
    /// Validates and constructs one query (fail-closed; the period bounds
    /// must parse as the recorded format and be ordered).
    pub fn new(
        in_domain: String,
        out_domain: String,
        period_start: String,
        period_end: String,
    ) -> Result<Self, A44QueryError> {
        if in_domain.trim().is_empty() || out_domain.trim().is_empty() {
            return Err(A44QueryError::EmptyArea);
        }
        let start = crate::document::parse_period_bound(&period_start)
            .map_err(|_| A44QueryError::InvalidPeriodBound {
                value: period_start.clone(),
            })?;
        let end = crate::document::parse_period_bound(&period_end)
            .map_err(|_| A44QueryError::InvalidPeriodBound {
                value: period_end.clone(),
            })?;
        if end <= start {
            return Err(A44QueryError::InvertedPeriod);
        }
        Ok(Self {
            in_domain,
            out_domain,
            period_start,
            period_end,
        })
    }

    /// The query parameters in canonical order (name, value) pairs —
    /// WITHOUT the security token. The token parameter is appended LAST
    /// at call time by the deployment, from the registered reference; it
    /// never passes through this crate's types (the [`token`] gate).
    pub fn parameters(&self) -> Vec<(&'static str, &str)> {
        vec![
            (A44_PARAM_DOCUMENT_TYPE, crate::document::A44_DOCUMENT_TYPE),
            (A44_PARAM_IN_DOMAIN, &self.in_domain),
            (A44_PARAM_OUT_DOMAIN, &self.out_domain),
            (A44_PARAM_PERIOD_START, &self.period_start),
            (A44_PARAM_PERIOD_END, &self.period_end),
        ]
    }
}

/// The response of one transport call. The document BODY is opaque text
/// at this boundary (the extraction seam parses it — `xml_seam`); the
/// transport carries bytes and a status, never an interpretation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct A44Response {
    /// The HTTP-ish status the transport reported (the recorded vocabulary
    /// is "ok" / "error"; the concrete client maps its own codes, UNVERIFIED).
    pub status: A44ResponseStatus,
    /// The raw document body bytes (opaque at this boundary).
    pub body: Vec<u8>,
}

/// The transport status vocabulary (closed).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum A44ResponseStatus {
    /// The platform returned a document.
    Ok,
    /// The platform refused or errored (the body carries the platform's
    /// error text; the adapter refuses the whole response, never trims).
    Error,
}

/// Why a transport call failed. Named, matchable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum A44TransportError {
    /// The transport seam is not wired (the DEFAULT posture — no HTTP
    /// client exists in this crate, and no request is sent in this
    /// program).
    SeamNotWired,
    /// The transport was configured but the call failed (code + message;
    /// never raw vendor text — the `BackendError` posture).
    CallFailed { code: String, message: String },
}

impl A44TransportError {
    /// The stable machine-readable error name.
    pub fn name(&self) -> &'static str {
        match self {
            A44TransportError::SeamNotWired => "A44_TRANSPORT_SEAM_NOT_WIRED",
            A44TransportError::CallFailed { .. } => "A44_TRANSPORT_CALL_FAILED",
        }
    }
}

impl std::fmt::Display for A44TransportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            A44TransportError::SeamNotWired => write!(
                f,
                "the ENTSO-E transport seam is not wired: no HTTP client exists in this \
                 crate and no request is sent in this program (the first live call is a \
                 human/lab gate after P15-E-2 closes)"
            ),
            A44TransportError::CallFailed { code, message } => {
                write!(f, "the transport call failed ({code}): {message}")
            }
        }
    }
}

impl std::error::Error for A44TransportError {}

/// The REST transport port (sync trait; the port-crate rule). The
/// composition root implements it over a real HTTP client ONLY after
/// P15-E-2 closes and a human registers the token. The port carries the
/// token parameter OUT OF BAND: an implementation resolves the registered
/// token reference at call time; the token value never passes through
/// this trait's types.
pub trait A44Transport {
    /// Executes one A44 query (READ-ONLY; there is no write method on
    /// this trait — structural).
    fn fetch_a44_document(&self, query: &A44PriceQuery) -> Result<A44Response, A44TransportError>;
}

/// The UNWIRED default transport: answers EVERY request with the named
/// `SeamNotWired` refusal. The absence of transport infrastructure can
/// never look like success (the P8.32 fail-closed posture, mirrored).
#[derive(Debug, Clone, Copy, Default)]
pub struct UnwiredA44Transport;

impl A44Transport for UnwiredA44Transport {
    fn fetch_a44_document(&self, _query: &A44PriceQuery) -> Result<A44Response, A44TransportError> {
        Err(A44TransportError::SeamNotWired)
    }
}

// ---------------------------------------------------------------------------
// Tests (WRITTEN-UNRUN — no cargo command was executed in this loop; the
// human build/test gate owns the run. Every expected value below is
// hand-derived from the rules above or a literal copied from this
// module's constants. Nothing is fabricated.)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_well_formed_query_constructs_and_names_the_recorded_parameters() {
        let query = A44PriceQuery::new(
            "FIXTURE-IN".to_string(),
            "FIXTURE-OUT".to_string(),
            "202606010000".to_string(),
            "202606020000".to_string(),
        )
        .expect("valid query");
        let params = query.parameters();
        // The recorded parameter names (UNVERIFIED per [S-111]; the
        // literals are pinned so a change is a deliberate, recorded one).
        assert_eq!(params[0].0, "documentType");
        assert_eq!(params[0].1, "A44");
        assert_eq!(params[1].0, "in_Domain");
        assert_eq!(params[2].0, "out_Domain");
        assert_eq!(params[3].0, "periodStart");
        assert_eq!(params[4].0, "periodEnd");
        // NO token parameter rides the typed parameters (the token is
        // out-of-band, resolved from the registered reference at call
        // time).
        assert!(!params.iter().any(|(name, _)| *name == A44_PARAM_SECURITY_TOKEN));
    }

    #[test]
    fn malformed_queries_are_refused() {
        let err = A44PriceQuery::new(
            "  ".to_string(),
            "OUT".to_string(),
            "202606010000".to_string(),
            "202606020000".to_string(),
        )
        .expect_err("empty area refused");
        assert_eq!(err.name(), "A44_QUERY_EMPTY_AREA");
        let err = A44PriceQuery::new(
            "IN".to_string(),
            "OUT".to_string(),
            "20260601".to_string(),
            "202606020000".to_string(),
        )
        .expect_err("malformed bound refused");
        assert_eq!(err.name(), "A44_QUERY_INVALID_PERIOD_BOUND");
        let err = A44PriceQuery::new(
            "IN".to_string(),
            "OUT".to_string(),
            "202606020000".to_string(),
            "202606010000".to_string(),
        )
        .expect_err("inverted period refused");
        assert_eq!(err.name(), "A44_QUERY_INVERTED_PERIOD");
    }

    #[test]
    fn the_default_transport_refuses_everything_fail_closed() {
        let query = A44PriceQuery::new(
            "IN".to_string(),
            "OUT".to_string(),
            "202606010000".to_string(),
            "202606020000".to_string(),
        )
        .expect("valid query");
        let err = UnwiredA44Transport
            .fetch_a44_document(&query)
            .expect_err("the unwired seam refuses");
        assert_eq!(err.name(), "A44_TRANSPORT_SEAM_NOT_WIRED");
        // The endpoint and parameter literals are pinned (UNVERIFIED per
        // [S-111]; a change is a deliberate, recorded one).
        assert_eq!(A44_QUERY_ENDPOINT, "https://transparency.entsoe.eu/api");
        assert_eq!(A44_PARAM_SECURITY_TOKEN, "securityToken");
    }

    #[test]
    fn every_transport_error_renders_non_empty_prose() {
        let errors = [
            A44QueryError::EmptyArea,
            A44QueryError::InvalidPeriodBound {
                value: "x".to_string(),
            },
            A44QueryError::InvertedPeriod,
            A44TransportError::SeamNotWired,
            A44TransportError::CallFailed {
                code: "E-TEST".to_string(),
                message: "fixture".to_string(),
            },
        ];
        for error in errors {
            assert!(!error.to_string().is_empty());
        }
    }
}
