// P6.10 — OpenEMS integration: EdgeConfig discovery and channel mapping
// (report §2.3, §18.1, §8.1, §27; ledger P6.10; decision record
// `ledger/phase-06-edge-protocol/10_11_openems.md`).
//
// WHAT THIS MODULE IS
//
// The `openems-rest` [`EmsAdapter`]: it fetches and parses the OpenEMS
// EdgeConfig JSON (components → channels, UNVERIFIED-ASSUMPTION shape — see
// below), classifies every discovered channel against a VERSIONED mapping
// profile (`crates/edge-agent/config/openems-mapping-profile.v1.json`),
// emits an unmapped/ambiguous-channel REGISTER (never a guess), and reads
// mapped channels over a minimal read-only HTTP/1.1 GET client.
//
// STRUCTURAL READ-ONLY GUARANTEE (P6.11 depends on this):
//
// The ONLY outbound verb on this surface is GET:
//
// - [`OpenemsRestTransport`] declares exactly ONE method (`get`);
// - [`TcpHttpGetTransport`] formats exactly one request line shape
//   (`GET <path> HTTP/1.1`);
// - the module contains NO other HTTP verb — no writer method, no
//   configuration call, nothing that mutates Edge state. A lexical scan
//   (`tests/openems_e2e_p6_11.rs`) enforces this by construction.
//
// EXECUTION-MODEL HONESTY (ledger P6.10 step 3): OpenEMS Edge runs an
// input–process–output cycle; a Cycle component sets the process image and
// executes Controllers in Scheduler order (RESEARCH.md §1). FlexCore READS
// channels through this adapter and NEVER injects into that cycle: this
// module enables or configures NO OpenEMS Controller, touches NO Scheduler,
// and writes NO setpoint. FlexCore is a supervisory orchestration layer;
// the OpenEMS input–process–output cycle stays entirely under the Edge's
// own control (AGENTS.md rule 3; report §18.1).
//
// UNVERIFIED-ASSUMPTION REGISTER (the API-shape honesty rule, AGENTS.md
// rule 1): RESEARCH.md §1 records the OpenEMS architecture (Natures,
// Channels, EdgeConfig as "the JSON definition of the Edge's
// configuration") but NO REST endpoint paths, NO JSON field names, and NO
// response shapes. Everything HTTP/JSON below is therefore labelled
// UNVERIFIED-ASSUMPTION with the OpenEMS documentation references (report
// §27: OpenEMS Edge architecture + Backend architecture; the OpenEMS
// REST/JSON-RPC API docs). The API was NEVER executed when this module was
// written. Each label marks exactly what the lab run (P6.11) must verify
// against the real Edge; a mismatch is a code or profile edit, never a
// silent reinterpretation. The version to pin in the lab is 2026.11.0 or
// whatever the pilot actually runs (RESEARCH.md §1 — component IDs and
// channels MOVE between monthly releases; 2026.11.0 removed Energy
// Scheduler v1).
//
// MAPPING RULES (fail-closed, ledger P6.10 acceptance criteria):
//
// - The mapping is a VERSIONED configuration file, not hardcoded. The
//   adapter refuses a profile with a version it does not support.
// - Every discovered channel is classified EXACTLY ONE of: MAPPED (valid,
//   unambiguous profile entry), EXPLICITLY_UNMAPPED (the profile declares
//   the channel unmapped WITH a reason), UNMAPPED (no profile entry —
//   recorded with that reason; never mapped by guessing), or AMBIGUOUS
//   (units/sign conflict, duplicate entries, unknown unit spelling,
//   missing sign/convention declaration — never mapped by guessing).
// - Units are never guessed: a source-declared unit that conflicts with
//   (or is unnameable against) the profile unit makes the channel
//   AMBIGUOUS. Sign conventions are declarations from the profile, never
//   inferences from observed signs (P0.07 §2–§3).
// - The register ([`OpenEmsRestAdapter::mapping_register`]) is populated at
//   discovery time and exposed for the decision record and for tests.
//
// QUALITY HONESTY: OpenEMS REST channel values carry NO quality code
// (UNVERIFIED-ASSUMPTION). There is NO default quality (P0.08): each
// profile mapping entry explicitly DECLARES the quality code the adapter
// attaches to a fresh reading of that channel; an entry that omits the
// declaration yields readings with NO quality code, which the ACL refuses
// (`AclError::QualityNotDeclared`). A reading older than the profile's
// `stale_after_millis` freshness limit is stamped with the `STALE` code,
// which the frozen ACL table maps to the canonical `STALE` state. `MISSING`
// is NEVER emitted by the edge: it is a platform-derived gap marker
// (P2.04 D-T6); downstream, the control plane's gap detector owns it.
//
// TLS HONESTY: this wave composes ONLY plain `http://` for the lab REST
// hop. A `https://` base URL is REFUSED with a named error — the rustls
// client for the REST hop is a recorded open item (decision record), not a
// silently downgraded trust model. The FlexCore edge↔central channel is
// unaffected: that is the P6.07 mTLS channel, a different path (RESEARCH.md
// contradiction note R6-C1 — FlexCore must not conflate its mTLS channel
// with OpenEMS's own API-key WebSocket).

use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use flexcore_contracts::time_model::ClockQuality;
use flexcore_contracts::units::ReactiveSignConvention;
use flexcore_ports::clock::Clock;
use flexcore_ports::telemetry_source::{
    QualityMappingDeclaration, SourceStateMapping, TelemetryQuantity,
};

use crate::adapters::acl::{
    AntiCorruptionLayer, NativeSign, NativeTimestamp, NativeUnit, PointMapping,
};
use crate::adapters::port::{
    AdapterCapabilities, AdapterError, AdapterHealth, ChannelDescriptor, EmsAdapter,
    EmsNativeReading, PointKey,
};

// --- Registered identity ------------------------------------------------------

/// The registered registry id of this adapter (`registry.rs`; AVAILABLE via
/// `AdapterRegistry::with_openems_available`, P6.10).
pub const OPENEMS_REST_ADAPTER_ID: &str = "openems-rest";

/// UNVERIFIED-ASSUMPTION — OpenEMS REST API, never executed; verify against
/// the lab Edge (RESEARCH.md §1; report §27 OpenEMS references). The
/// EdgeConfig fetch path this adapter uses; the real lab Edge may expose a
/// different path or a JSON-RPC shape instead.
pub const OPENEMS_EDGE_CONFIG_PATH: &str = "/rest/edge/config";

/// UNVERIFIED-ASSUMPTION — OpenEMS REST API, never executed; verify against
/// the lab Edge. The channel-value path prefix; the adapter builds
/// `{prefix}{component}/{channel}` and requires both ids to be
/// REST-path-safe (see [`is_rest_path_safe`]).
pub const OPENEMS_CHANNEL_PATH_PREFIX: &str = "/rest/channel/";

/// The quality code the adapter attaches when a reading is older than the
/// profile's freshness limit. This is one of the frozen ACL table's named
/// codes (STALE → canonical `STALE`); it is an adapter-derived code from an
/// EXPLICIT declared freshness limit — never a guessed quality.
pub const OPENEMS_STALE_CODE: &str = "STALE";

/// The profile version this adapter understands. A different major shape is
/// a reviewed migration, never an interpreted guess.
pub const SUPPORTED_PROFILE_VERSION: u32 = 1;

/// The profile identity string this adapter requires (guards loading a
/// foreign JSON file as a mapping profile).
pub const PROFILE_ID: &str = "openems-mapping-profile";

// --- Errors -------------------------------------------------------------------

/// Errors of the OpenEMS adapter surface. Every failure is named; nothing
/// is swallowed, and no error message ever echoes the configured base URL
/// (it may embed credentials — the `config.rs` redaction rule).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpenemsError {
    /// A response body was not valid JSON.
    Json { offset: usize, reason: String },
    /// Reading the mapping profile file failed.
    Io { reason: String },
    /// The mapping profile is structurally unusable (wrong identity,
    /// unsupported version, missing/invalid required field). Fail-closed.
    ProfileEnvelope { reason: String },
    /// The REST transport failed (connect, timeout, malformed response
    /// framing).
    Transport { reason: String },
    /// The endpoint answered with a non-success HTTP status.
    HttpStatus { status: u16, path: String },
    /// The configured base URL is unusable (wrong scheme, bad host/port).
    /// The URL VALUE is never included — it may carry credentials.
    BaseUrl { reason: String },
}

impl std::fmt::Display for OpenemsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OpenemsError::Json { offset, reason } => {
                write!(f, "invalid JSON at byte offset {offset}: {reason}")
            }
            OpenemsError::Io { reason } => write!(f, "mapping profile I/O failed: {reason}"),
            OpenemsError::ProfileEnvelope { reason } => {
                write!(f, "the mapping profile is refused (fail-closed): {reason}")
            }
            OpenemsError::Transport { reason } => {
                write!(f, "the OpenEMS REST transport failed: {reason}")
            }
            OpenemsError::HttpStatus { status, path } => {
                write!(f, "the endpoint '{path}' returned HTTP status {status}")
            }
            OpenemsError::BaseUrl { reason } => {
                write!(f, "the configured OpenEMS base URL is refused: {reason}")
            }
        }
    }
}

impl std::error::Error for OpenemsError {}

// --- Minimal std-only JSON ------------------------------------------------------

// No new dependencies were permitted in this wave (loop bans; the P6.02
// audit note), so the adapter carries its own minimal JSON reader. It
// parses the shapes the OpenEMS surfaces use (objects, arrays, strings with
// escapes and surrogate pairs, numbers, booleans, null) with a 64-level
// depth bound; duplicate object keys keep the FIRST occurrence (`get`).
// Numbers are f64 — values beyond 2^53 lose integer precision, a recorded
// limitation (EdgeConfig identifiers are strings; lab telemetry ranges are
// far below the limit).

mod json {
    /// A parsed JSON value.
    #[derive(Debug, Clone, PartialEq)]
    pub enum Json {
        Null,
        Bool(bool),
        Number(f64),
        String(String),
        Array(Vec<Json>),
        /// Object members in document order; duplicates keep the first.
        Object(Vec<(String, Json)>),
    }

    /// A named JSON parse failure (offset + reason).
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct JsonError {
        pub offset: usize,
        pub reason: String,
    }

    impl std::fmt::Display for JsonError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "invalid JSON at byte offset {}: {}", self.offset, self.reason)
        }
    }

    const MAX_DEPTH: usize = 64;

    /// Parses a complete JSON document (trailing garbage is refused).
    pub fn parse(text: &str) -> Result<Json, JsonError> {
        let mut parser = Parser {
            bytes: text.as_bytes(),
            pos: 0,
            depth: 0,
        };
        parser.skip_ws();
        let value = parser.value()?;
        parser.skip_ws();
        if parser.pos != parser.bytes.len() {
            return Err(parser.error("trailing characters after the JSON document"));
        }
        Ok(value)
    }

    impl Json {
        /// The FIRST member with this key (objects keep document order).
        pub fn get(&self, key: &str) -> Option<&Json> {
            match self {
                Json::Object(members) => {
                    members.iter().find(|(k, _)| k == key).map(|(_, v)| v)
                }
                _ => None,
            }
        }

        pub fn as_str(&self) -> Option<&str> {
            match self {
                Json::String(s) => Some(s),
                _ => None,
            }
        }

        pub fn as_f64(&self) -> Option<f64> {
            match self {
                Json::Number(n) => Some(*n),
                _ => None,
            }
        }

        pub fn as_array(&self) -> Option<&[Json]> {
            match self {
                Json::Array(items) => Some(items),
                _ => None,
            }
        }

        pub fn as_object(&self) -> Option<&[(String, Json)]> {
            match self {
                Json::Object(members) => Some(members),
                _ => None,
            }
        }
    }

    struct Parser<'a> {
        bytes: &'a [u8],
        pos: usize,
        depth: usize,
    }

    impl<'a> Parser<'a> {
        fn error(&self, reason: impl Into<String>) -> JsonError {
            JsonError {
                offset: self.pos,
                reason: reason.into(),
            }
        }

        fn peek(&self) -> Option<u8> {
            self.bytes.get(self.pos).copied()
        }

        fn bump(&mut self) -> Option<u8> {
            let byte = self.peek();
            if byte.is_some() {
                self.pos += 1;
            }
            byte
        }

        fn skip_ws(&mut self) {
            while matches!(self.peek(), Some(b' ' | b'\t' | b'\r' | b'\n')) {
                self.pos += 1;
            }
        }

        fn expect(&mut self, byte: u8, what: &str) -> Result<(), JsonError> {
            if self.bump() == Some(byte) {
                Ok(())
            } else {
                Err(self.error(format!("expected {what}")))
            }
        }

        fn literal(&mut self, word: &str, value: Json) -> Result<Json, JsonError> {
            if self.bytes[self.pos..].starts_with(word.as_bytes()) {
                self.pos += word.len();
                Ok(value)
            } else {
                Err(self.error(format!("invalid literal; expected '{word}'")))
            }
        }

        fn value(&mut self) -> Result<Json, JsonError> {
            self.depth += 1;
            if self.depth > MAX_DEPTH {
                self.depth -= 1;
                return Err(self.error("JSON nesting exceeds the 64-level safety bound"));
            }
            let out = match self.peek() {
                Some(b'{') => self.object(),
                Some(b'[') => self.array(),
                Some(b'"') => Ok(Json::String(self.string()?)),
                Some(b't') => self.literal("true", Json::Bool(true)),
                Some(b'f') => self.literal("false", Json::Bool(false)),
                Some(b'n') => self.literal("null", Json::Null),
                Some(c) if c == b'-' || c.is_ascii_digit() => self.number(),
                Some(_) => Err(self.error("unexpected character")),
                None => Err(self.error("unexpected end of input")),
            };
            self.depth -= 1;
            out
        }

        fn object(&mut self) -> Result<Json, JsonError> {
            self.expect(b'{', "the start of an object")?;
            let mut members: Vec<(String, Json)> = Vec::new();
            self.skip_ws();
            if self.peek() == Some(b'}') {
                self.pos += 1;
                return Ok(Json::Object(members));
            }
            loop {
                self.skip_ws();
                let key = self.string()?;
                self.skip_ws();
                self.expect(b':', "':' after an object key")?;
                let value = self.value()?;
                members.push((key, value));
                self.skip_ws();
                match self.bump() {
                    Some(b',') => continue,
                    Some(b'}') => return Ok(Json::Object(members)),
                    _ => return Err(self.error("expected ',' or '}' inside an object")),
                }
            }
        }

        fn array(&mut self) -> Result<Json, JsonError> {
            self.expect(b'[', "the start of an array")?;
            let mut items = Vec::new();
            self.skip_ws();
            if self.peek() == Some(b']') {
                self.pos += 1;
                return Ok(Json::Array(items));
            }
            loop {
                let value = self.value()?;
                items.push(value);
                self.skip_ws();
                match self.bump() {
                    Some(b',') => continue,
                    Some(b']') => return Ok(Json::Array(items)),
                    _ => return Err(self.error("expected ',' or ']' inside an array")),
                }
            }
        }

        fn number(&mut self) -> Result<Json, JsonError> {
            let start = self.pos;
            while let Some(c) = self.peek() {
                if c.is_ascii_digit() || matches!(c, b'-' | b'+' | b'.' | b'e' | b'E') {
                    self.pos += 1;
                } else {
                    break;
                }
            }
            let token = std::str::from_utf8(&self.bytes[start..self.pos])
                .map_err(|_| self.error("invalid number encoding"))?;
            token
                .parse::<f64>()
                .map(Json::Number)
                .map_err(|_| self.error(format!("'{token}' is not a valid JSON number")))
        }

        fn string(&mut self) -> Result<String, JsonError> {
            self.expect(b'"', "the opening quote of a string")?;
            let mut out = String::new();
            loop {
                match self.bump() {
                    None => return Err(self.error("unterminated string")),
                    Some(b'"') => return Ok(out),
                    Some(b'\\') => match self.bump() {
                        None => return Err(self.error("unterminated escape sequence")),
                        Some(b'"') => out.push('"'),
                        Some(b'\\') => out.push('\\'),
                        Some(b'/') => out.push('/'),
                        Some(b'b') => out.push('\u{0008}'),
                        Some(b'f') => out.push('\u{000C}'),
                        Some(b'n') => out.push('\n'),
                        Some(b'r') => out.push('\r'),
                        Some(b't') => out.push('\t'),
                        Some(b'u') => {
                            let high = self.hex4()?;
                            let ch = if (0xD800..0xDC00).contains(&high) {
                                // High surrogate: a low surrogate MUST follow.
                                if self.bump() != Some(b'\\') {
                                    return Err(self.error("lone high surrogate"));
                                }
                                if self.bump() != Some(b'u') {
                                    return Err(self.error("lone high surrogate"));
                                }
                                let low = self.hex4()?;
                                if !(0xDC00..0xE000).contains(&low) {
                                    return Err(self.error("invalid low surrogate"));
                                }
                                let combined =
                                    0x10000 + ((high - 0xD800) << 10) + (low - 0xDC00);
                                char::from_u32(combined)
                                    .ok_or_else(|| self.error("invalid surrogate pair"))?
                            } else if (0xDC00..0xE000).contains(&high) {
                                return Err(self.error("lone low surrogate"));
                            } else {
                                char::from_u32(high)
                                    .ok_or_else(|| self.error("invalid \\u escape"))?
                            };
                            out.push(ch);
                        }
                        Some(_) => return Err(self.error("unknown escape character")),
                    },
                    Some(c) if c < 0x20 => {
                        return Err(self.error("raw control character inside a string"))
                    }
                    Some(c) if c < 0x80 => out.push(c as char),
                    // Multi-byte UTF-8: the input is a valid &str, so the
                    // whole sequence is copied verbatim.
                    Some(lead) => {
                        let len = match lead {
                            0xC0..=0xDF => 2,
                            0xE0..=0xEF => 3,
                            0xF0..=0xF7 => 4,
                            _ => return Err(self.error("invalid UTF-8 lead byte")),
                        };
                        let end = self.pos + len - 1;
                        if end > self.bytes.len() {
                            return Err(self.error("truncated UTF-8 sequence"));
                        }
                        let sequence = std::str::from_utf8(&self.bytes[self.pos - 1..end])
                            .map_err(|_| self.error("invalid UTF-8 sequence"))?;
                        out.push_str(sequence);
                        self.pos = end;
                    }
                }
            }
        }

        fn hex4(&mut self) -> Result<u32, JsonError> {
            let mut value = 0u32;
            for _ in 0..4 {
                let byte = self
                    .bump()
                    .ok_or_else(|| self.error("truncated \\u escape"))?;
                let digit = (byte as char)
                    .to_digit(16)
                    .ok_or_else(|| self.error("invalid hex digit in \\u escape"))?;
                value = value * 16 + digit;
            }
            Ok(value)
        }
    }
}

// --- The read-only transport seam ---------------------------------------------

/// One REST response (status line + body). Read-only by construction:
/// there is no request-body type anywhere in this module.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RestResponse {
    pub status: u16,
    pub body: String,
}

/// The transport seam for the OpenEMS REST surface. THE ONLY OUTBOUND VERB
/// IS `get` — this trait declares no other method, so no caller of this
/// module can express a state-changing request against the Edge. The
/// lexical scan in `tests/openems_e2e_p6_11.rs` enforces the same guarantee
/// over the module source.
pub trait OpenemsRestTransport: Send {
    /// Performs ONE read-only GET and returns the status + body.
    fn get(&mut self, path: &str) -> Result<RestResponse, OpenemsError>;
}

/// A minimal read-only HTTP/1.1 GET client over `std::net::TcpStream`
/// (blocking; the P6.12 house rule: a concrete adapter owns its I/O shape
/// and bridges to the sync port). One connection per GET
/// (`Connection: close`) — the lab poll cadence is supervisory
/// (minutes-scale, §3.5), so no connection pool is warranted.
///
/// Scheme policy (fail-closed): ONLY `http://` composes this wave. A
/// `https://` base URL is a NAMED refusal — the TLS client for the REST hop
/// is a recorded open item (decision record §6), not a silently downgraded
/// trust model. The FlexCore edge↔central uplink remains the P6.07 mTLS
/// channel; the two are different paths (RESEARCH.md R6-C1).
pub struct TcpHttpGetTransport {
    host: String,
    port: u16,
    connect_timeout: Duration,
    read_timeout: Duration,
}

impl TcpHttpGetTransport {
    /// Parses `http://host[:port]` (default port 80). The URL value is
    /// never echoed in errors (credential redaction rule).
    pub fn from_base_url(
        base_url: &str,
        connect_timeout: Duration,
        read_timeout: Duration,
    ) -> Result<Self, OpenemsError> {
        let trimmed = base_url.trim();
        const SCHEME: &str = "http://";
        let rest = trimmed.strip_prefix(SCHEME).ok_or_else(|| {
            OpenemsError::BaseUrl {
                reason: "only the 'http://' scheme is composable this wave; the TLS client \
                         for the OpenEMS REST hop is a recorded open item — a TLS endpoint \
                         is refused fail-closed, never downgraded"
                    .to_string(),
            }
        })?;
        if rest.is_empty() {
            return Err(OpenemsError::BaseUrl {
                reason: "the host part is empty".to_string(),
            });
        }
        let (host, port) = match rest.rsplit_once(':') {
            Some((host, port_text)) => {
                let port = port_text.parse::<u16>().map_err(|_| OpenemsError::BaseUrl {
                    reason: "the port is not a valid u16".to_string(),
                })?;
                (host.to_string(), port)
            }
            None => (rest.to_string(), 80),
        };
        if host.is_empty() || host.contains('/') || host.contains('@') {
            return Err(OpenemsError::BaseUrl {
                reason: "the host part is not a plain host (no path, no userinfo)".to_string(),
            });
        }
        if connect_timeout.is_zero() || read_timeout.is_zero() {
            return Err(OpenemsError::BaseUrl {
                reason: "timeouts must be positive".to_string(),
            });
        }
        Ok(Self {
            host,
            port,
            connect_timeout,
            read_timeout,
        })
    }

    /// The host (diagnostics; never a secret).
    pub fn host(&self) -> &str {
        &self.host
    }

    /// The port (diagnostics).
    pub fn port(&self) -> u16 {
        self.port
    }
}

impl OpenemsRestTransport for TcpHttpGetTransport {
    fn get(&mut self, path: &str) -> Result<RestResponse, OpenemsError> {
        if !path.starts_with('/')
            || path.chars().any(|c| matches!(c, '\r' | '\n' | ' ' | '\t' | '?'))
        {
            return Err(OpenemsError::Transport {
                reason: "the request path must start with '/' and carry no whitespace, \
                         control characters, or query part"
                    .to_string(),
            });
        }
        let address = (self.host.as_str(), self.port)
            .to_socket_addrs()
            .map_err(|error| OpenemsError::Transport {
                reason: format!("the host could not be resolved: {error}"),
            })?
            .next()
            .ok_or_else(|| OpenemsError::Transport {
                reason: "the host resolved to no addresses".to_string(),
            })?;
        let mut stream =
            TcpStream::connect_timeout(&address, self.connect_timeout).map_err(|error| {
                OpenemsError::Transport {
                    reason: format!("the connection to the Edge failed: {error}"),
                }
            })?;
        stream
            .set_read_timeout(Some(self.read_timeout))
            .map_err(|error| OpenemsError::Transport {
                reason: format!("the read timeout could not be set: {error}"),
            })?;
        stream
            .set_write_timeout(Some(self.connect_timeout))
            .map_err(|error| OpenemsError::Transport {
                reason: format!("the write timeout could not be set: {error}"),
            })?;
        // The ONLY request this module ever formats: a read-only GET with
        // `Connection: close` (one request per connection).
        let request = format!(
            "GET {path} HTTP/1.1\r\nHost: {host}:{port}\r\nAccept: application/json\r\n\
             Connection: close\r\nUser-Agent: flexcore-edge-agent-openems-rest\r\n\r\n",
            host = self.host,
            port = self.port,
        );
        stream
            .write_all(request.as_bytes())
            .map_err(|error| OpenemsError::Transport {
                reason: format!("the request could not be sent: {error}"),
            })?;
        let mut raw = Vec::new();
        stream
            .read_to_end(&mut raw)
            .map_err(|error| OpenemsError::Transport {
                reason: format!("the response could not be read: {error}"),
            })?;
        let split = find_double_crlf(&raw).ok_or_else(|| OpenemsError::Transport {
            reason: "the response has no header/body separator".to_string(),
        })?;
        let head = String::from_utf8_lossy(&raw[..split]);
        let status_line = head
            .lines()
            .next()
            .ok_or_else(|| OpenemsError::Transport {
                reason: "the response has no status line".to_string(),
            })?;
        // "HTTP/1.1 200 OK" → the second space-separated token.
        let status = status_line
            .split_ascii_whitespace()
            .nth(1)
            .and_then(|code| code.parse::<u16>().ok())
            .ok_or_else(|| OpenemsError::Transport {
                reason: "the status line carries no parseable status code".to_string(),
            })?;
        let body = String::from_utf8_lossy(&raw[split + 4..]).to_string();
        Ok(RestResponse { status, body })
    }
}

/// Finds the `\r\n\r\n` header/body separator.
fn find_double_crlf(raw: &[u8]) -> Option<usize> {
    raw.windows(4).position(|window| window == b"\r\n\r\n")
}

// --- The versioned mapping profile ---------------------------------------------

/// One mapping-profile entry (raw declared spellings; validated at channel
/// classification, where failures become AMBIGUOUS register entries — never
/// guesses).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileMapping {
    pub component_id: String,
    pub channel_id: String,
    pub asset_key: String,
    /// The canonical quantity spelling (`ActivePower` | `ReactivePower` |
    /// `Energy` | `Soc`).
    pub quantity: String,
    /// The native unit spelling (must be in the ACL's named unit table).
    pub native_unit: String,
    /// Required for `ActivePower` entries (`export-positive` |
    /// `import-positive`).
    pub native_sign: Option<String>,
    /// Optional per-entry reactive convention override
    /// (`injection-positive` | `consumption-positive`); absent = the
    /// profile's top-level declared convention applies.
    pub reactive_source_convention: Option<String>,
    /// UNVERIFIED-ASSUMPTION: OpenEMS REST channel values carry no quality
    /// code; each entry DECLARES the code the adapter attaches to a FRESH
    /// reading of this channel (an explicit, versioned, reviewed
    /// declaration — never an implicit default). Absent = the ACL refuses
    /// the reading (no quality), which is the fail-closed behaviour.
    pub declared_quality_code: Option<String>,
}

/// One profile-declared UNMAPPED channel, with its recorded reason.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileUnmapped {
    pub component_id: String,
    pub channel_id: String,
    pub reason: String,
}

/// The versioned channel-mapping profile (ledger P6.10 step 4: the mapping
/// is a versioned configuration file, not hardcoded in the adapter).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenemsMappingProfile {
    version: u32,
    reactive_source_convention: ReactiveSignConvention,
    cadence_millis: u64,
    stale_after: Duration,
    assumptions: Vec<String>,
    mappings: Vec<ProfileMapping>,
    declared_unmapped: Vec<ProfileUnmapped>,
}

impl OpenemsMappingProfile {
    /// Parses the profile from a JSON document (the exact envelope this
    /// adapter accepts is documented by this function; anything else is a
    /// named refusal — never interpreted leniently).
    pub fn from_json_str(text: &str) -> Result<Self, OpenemsError> {
        let document = json::parse(text).map_err(|error| OpenemsError::Json {
            offset: error.offset,
            reason: error.reason,
        })?;
        let envelope_error = |reason: String| OpenemsError::ProfileEnvelope { reason };

        let identity = obj_str(&document, "profile", "the profile")?;
        if identity != PROFILE_ID {
            return Err(envelope_error(format!(
                "the file identifies itself as '{identity}', not '{PROFILE_ID}'"
            )));
        }
        let version = obj_num(&document, "version", "the profile")?;
        if version.fract() != 0.0 || (version as u32) != SUPPORTED_PROFILE_VERSION {
            return Err(envelope_error(format!(
                "profile version {version} is not supported (this adapter understands \
                 version {SUPPORTED_PROFILE_VERSION} only; a different version is a \
                 reviewed migration, never a guessed interpretation)"
            )));
        }
        let convention_spelling = obj_str(
            &document,
            "reactive_source_convention",
            "the profile",
        )?;
        let reactive_source_convention =
            parse_convention(convention_spelling).ok_or_else(|| {
                envelope_error(format!(
                    "unknown reactive_source_convention '{convention_spelling}' \
                     (expected 'injection-positive' or 'consumption-positive')"
                ))
            })?;
        let cadence_millis = obj_num(&document, "cadence_millis", "the profile")?;
        if cadence_millis.fract() != 0.0 || cadence_millis <= 0.0 {
            return Err(envelope_error(
                "cadence_millis must be a positive whole number of milliseconds".to_string(),
            ));
        }
        let stale_after_millis = obj_num(&document, "stale_after_millis", "the profile")?;
        if stale_after_millis.fract() != 0.0 || stale_after_millis <= 0.0 {
            return Err(envelope_error(
                "stale_after_millis must be a positive whole number of milliseconds"
                    .to_string(),
            ));
        }

        let mappings_json = document.get("mappings").and_then(|v| v.as_array()).ok_or_else(
            || {
                envelope_error(
                    "the profile carries no 'mappings' array (required, non-empty)".to_string(),
                )
            },
        )?;
        if mappings_json.is_empty() {
            return Err(envelope_error(
                "the profile declares no mappings; a read path with nothing mapped is a \
                 composition failure, not a silent no-op"
                    .to_string(),
            ));
        }
        let mut mappings = Vec::new();
        for (index, entry) in mappings_json.iter().enumerate() {
            let context = format!("mapping entry #{index}");
            let mut raw = ProfileMapping {
                component_id: obj_str(entry, "component_id", &context)?.to_string(),
                channel_id: obj_str(entry, "channel_id", &context)?.to_string(),
                asset_key: obj_str(entry, "asset_key", &context)?.to_string(),
                quantity: obj_str(entry, "quantity", &context)?.to_string(),
                native_unit: obj_str(entry, "native_unit", &context)?.to_string(),
                native_sign: opt_str(entry, "native_sign").map(str::to_string),
                reactive_source_convention: opt_str(entry, "reactive_source_convention")
                    .map(str::to_string),
                declared_quality_code: opt_str(entry, "declared_quality_code")
                    .map(str::to_string),
            };
            for (field, value) in [
                ("component_id", &raw.component_id),
                ("channel_id", &raw.channel_id),
                ("asset_key", &raw.asset_key),
                ("quantity", &raw.quantity),
                ("native_unit", &raw.native_unit),
            ] {
                if value.trim().is_empty() {
                    return Err(envelope_error(format!(
                        "{context}: field '{field}' is empty"
                    )));
                }
            }
            raw.component_id = raw.component_id.trim().to_string();
            raw.channel_id = raw.channel_id.trim().to_string();
            raw.asset_key = raw.asset_key.trim().to_string();
            mappings.push(raw);
        }

        let mut declared_unmapped = Vec::new();
        if let Some(unmapped_json) = document.get("declared_unmapped").and_then(|v| v.as_array())
        {
            for (index, entry) in unmapped_json.iter().enumerate() {
                let context = format!("declared_unmapped entry #{index}");
                declared_unmapped.push(ProfileUnmapped {
                    component_id: obj_str(entry, "component_id", &context)?.to_string(),
                    channel_id: obj_str(entry, "channel_id", &context)?.to_string(),
                    reason: obj_str(entry, "reason", &context)?.to_string(),
                });
            }
        }

        let mut assumptions = Vec::new();
        if let Some(assumption_json) = document.get("assumptions").and_then(|v| v.as_array()) {
            for item in assumption_json {
                match item.as_str() {
                    Some(text) => assumptions.push(text.to_string()),
                    None => {
                        return Err(envelope_error(
                            "assumptions entries must be strings".to_string(),
                        ))
                    }
                }
            }
        }

        Ok(Self {
            version: SUPPORTED_PROFILE_VERSION,
            reactive_source_convention,
            cadence_millis: cadence_millis as u64,
            stale_after: Duration::from_millis(stale_after_millis as u64),
            assumptions,
            mappings,
            declared_unmapped,
        })
    }

    /// Reads and parses the profile from a file (named I/O errors).
    pub fn from_path(path: impl AsRef<Path>) -> Result<Self, OpenemsError> {
        let text = std::fs::read_to_string(path.as_ref()).map_err(|error| {
            OpenemsError::Io {
                reason: format!(
                    "the mapping profile could not be read from {}: {error}",
                    path.as_ref().display()
                ),
            }
        })?;
        Self::from_json_str(&text)
    }

    pub fn version(&self) -> u32 {
        self.version
    }

    pub fn reactive_source_convention(&self) -> ReactiveSignConvention {
        self.reactive_source_convention
    }

    /// The declared poll cadence.
    pub fn cadence(&self) -> Duration {
        Duration::from_millis(self.cadence_millis)
    }

    /// The declared freshness limit: a reading older than this (measured
    /// against the edge clock at read time) is stamped `STALE`.
    pub fn stale_after(&self) -> Duration {
        self.stale_after
    }

    /// The recorded UNVERIFIED-ASSUMPTION strings the profile carries
    /// (exposed for the decision record; never silently dropped).
    pub fn assumptions(&self) -> &[String] {
        &self.assumptions
    }

    pub fn mappings(&self) -> &[ProfileMapping] {
        &self.mappings
    }

    pub fn declared_unmapped(&self) -> &[ProfileUnmapped] {
        &self.declared_unmapped
    }
}

fn obj_str<'a>(
    value: &'a json::Json,
    key: &str,
    context: &str,
) -> Result<&'a str, OpenemsError> {
    let envelope = |reason: String| OpenemsError::ProfileEnvelope { reason };
    let field = value
        .get(key)
        .ok_or_else(|| envelope(format!("{context}: missing required field '{key}'")))?;
    field
        .as_str()
        .ok_or_else(|| envelope(format!("{context}: field '{key}' must be a string")))
}

fn obj_num(value: &json::Json, key: &str, context: &str) -> Result<f64, OpenemsError> {
    let envelope = |reason: String| OpenemsError::ProfileEnvelope { reason };
    let field = value
        .get(key)
        .ok_or_else(|| envelope(format!("{context}: missing required field '{key}'")))?;
    field
        .as_f64()
        .ok_or_else(|| envelope(format!("{context}: field '{key}' must be a number")))
}

fn opt_str<'a>(value: &'a json::Json, key: &str) -> Option<&'a str> {
    value.get(key).and_then(|field| field.as_str())
}

// --- Spelling parsers (declarations, never inferences) --------------------------

fn parse_quantity(spelling: &str) -> Option<TelemetryQuantity> {
    match spelling.trim() {
        "ActivePower" => Some(TelemetryQuantity::ActivePower),
        "ReactivePower" => Some(TelemetryQuantity::ReactivePower),
        "Energy" => Some(TelemetryQuantity::Energy),
        "Soc" => Some(TelemetryQuantity::Soc),
        _ => None,
    }
}

fn parse_native_sign(spelling: &str) -> Option<NativeSign> {
    match spelling.trim() {
        "export-positive" => Some(NativeSign::ExportPositive),
        "import-positive" => Some(NativeSign::ImportPositive),
        _ => None,
    }
}

fn parse_convention(spelling: &str) -> Option<ReactiveSignConvention> {
    match spelling.trim() {
        "injection-positive" => Some(ReactiveSignConvention::InjectionPositive),
        "consumption-positive" => Some(ReactiveSignConvention::ConsumptionPositive),
        _ => None,
    }
}

fn convention_spelling(convention: ReactiveSignConvention) -> &'static str {
    match convention {
        ReactiveSignConvention::InjectionPositive => "injection-positive",
        ReactiveSignConvention::ConsumptionPositive => "consumption-positive",
    }
}

/// The unit family the ACL pairs with quantities (mirror of the ACL's
/// private family rule, used so a mismatched profile entry is classified
/// AMBIGUOUS at discovery instead of surfacing as a composition surprise).
fn unit_family(unit: NativeUnit) -> &'static str {
    match unit {
        NativeUnit::W | NativeUnit::Kw | NativeUnit::Mw => "power",
        NativeUnit::Var | NativeUnit::Kvar | NativeUnit::Mvar => "reactive",
        NativeUnit::Wh | NativeUnit::Kwh | NativeUnit::Mwh => "energy",
        NativeUnit::SocPercent | NativeUnit::SocFraction => "soc",
    }
}

/// Whether an id can be embedded verbatim in a REST path (no separators,
/// no whitespace, no control characters — the ids come from the Edge's
/// JSON, which is untrusted input).
fn is_rest_path_safe(id: &str) -> bool {
    !id.is_empty()
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
}

// --- The unmapped/ambiguous-channel register ------------------------------------

/// The classification of ONE discovered channel (the P6.10 register).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChannelMappingStatus {
    /// A valid, unambiguous profile entry maps this channel.
    Mapped,
    /// The profile explicitly declares the channel unmapped, with a reason.
    ExplicitlyUnmapped { reason: String },
    /// No profile entry exists; the channel is recorded unmapped with that
    /// reason — never mapped by guessing.
    Unmapped { reason: String },
    /// The units/signs/declarations are ambiguous; the channel is flagged
    /// and NOT mapped (no guessing, P6.10 acceptance criteria).
    Ambiguous { reason: String },
}

/// One register entry: the point, its classification, what the Edge itself
/// declared about its unit (diagnostic; UNVERIFIED-ASSUMPTION key), and a
/// human-safe description.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChannelMappingEntry {
    pub point: PointKey,
    pub status: ChannelMappingStatus,
    /// The unit string the EdgeConfig declared for the channel, if any.
    pub source_declared_unit: Option<String>,
    /// Human-safe description (never raw backend text — the P1.08 rule).
    pub description: String,
}

// --- The adapter -----------------------------------------------------------------

/// The per-point metadata a successful discovery builds (the validated
/// form of one profile entry; the source of the ACL's `PointMapping`s).
#[derive(Debug, Clone, PartialEq)]
struct MappedPointMeta {
    point: PointKey,
    asset_key: String,
    quantity: TelemetryQuantity,
    native_unit: NativeUnit,
    native_sign: Option<NativeSign>,
    reactive_convention: Option<ReactiveSignConvention>,
    declared_quality_code: Option<String>,
    /// Monotonically increasing per point, incremented on each successful
    /// read (P0.09 §1 source sequence).
    sequence: u64,
}

/// The `openems-rest` [`EmsAdapter`]: EdgeConfig discovery + versioned
/// channel mapping + read-only channel reads. Read-only by construction —
/// see the module header; the transport trait exposes GET only.
pub struct OpenEmsRestAdapter {
    transport: Box<dyn OpenemsRestTransport>,
    profile: OpenemsMappingProfile,
    clock: Arc<dyn Clock + Send + Sync>,
    capabilities: AdapterCapabilities,
    /// Discovered mapped points (empty until a discovery succeeds).
    mapped: HashMap<PointKey, MappedPointMeta>,
    /// The full classification register (empty until a discovery succeeds).
    register: Vec<ChannelMappingEntry>,
    /// Whether a discovery has succeeded (reads refuse before it).
    discovered: bool,
    health: AdapterHealth,
}

impl OpenEmsRestAdapter {
    /// Composes the adapter over a transport, a loaded profile, and the
    /// edge clock (used for the freshness limit). Construction validates
    /// the profile's quality declarations fail-closed: every code the
    /// adapter can report must map through the frozen ACL table.
    pub fn new(
        transport: Box<dyn OpenemsRestTransport>,
        profile: OpenemsMappingProfile,
        clock: Arc<dyn Clock + Send + Sync>,
    ) -> Result<Self, OpenemsError> {
        // The DECLARED quality mapping (P0.08 §3): every code the adapter
        // can attach to a reading — the entries' declared fresh-reading
        // codes plus the adapter-derived STALE code — mapped through the
        // frozen ACL table. A declared code the table refuses fails
        // construction (never a guessed canonical state).
        let mut codes: Vec<String> = vec![OPENEMS_STALE_CODE.to_string()];
        for mapping in profile.mappings() {
            if let Some(code) = &mapping.declared_quality_code {
                let code = code.trim().to_string();
                if !code.is_empty() && !codes.contains(&code) {
                    codes.push(code);
                }
            }
        }
        let quality_lookup = AntiCorruptionLayer::new(
            Vec::new(),
            "edge:openems-profile-quality-declaration",
            Vec::new(),
        )
        .map_err(|error| OpenemsError::ProfileEnvelope {
            reason: format!("the quality-declaration lookup could not be built: {error}"),
        })?;
        let mut declared = Vec::new();
        for code in &codes {
            let canonical = quality_lookup
                .map_quality_code(Some(code))
                .map_err(|error| OpenemsError::ProfileEnvelope {
                    reason: format!(
                        "the profile declares quality code '{code}' which the frozen ACL \
                         table refuses: {error}"
                    ),
                })?;
            declared.push(SourceStateMapping {
                source_code: code.clone(),
                canonical,
            });
        }
        let quality_mapping = QualityMappingDeclaration::new(declared)
            .map_err(|error| OpenemsError::ProfileEnvelope {
                reason: format!("the declared quality mapping was refused: {error}"),
            })?;
        let capabilities = AdapterCapabilities {
            adapter_id: OPENEMS_REST_ADAPTER_ID.to_string(),
            // The profile's top-level DECLARED reactive sign convention
            // (P0.07 §3 — a declaration, never inferred).
            reactive_sign_convention: profile.reactive_source_convention(),
            cadence: profile.cadence(),
            quality_mapping,
            // The Edge's clock synchronisation status is UNKNOWN until the
            // lab run records it (P0.09 §4 — recorded, never assumed good).
            clock_quality: ClockQuality::Unknown,
        };
        Ok(Self {
            transport,
            profile,
            clock,
            capabilities,
            mapped: HashMap::new(),
            register: Vec::new(),
            discovered: false,
            health: AdapterHealth::Unreachable {
                reason: "EdgeConfig discovery has not run successfully yet".to_string(),
            },
        })
    }

    /// The classification register populated by the last successful
    /// discovery (the P6.10 unmapped/ambiguous-channel register — exposed
    /// for the decision record and for tests).
    pub fn mapping_register(&self) -> &[ChannelMappingEntry] {
        &self.register
    }

    /// The validated ACL mapping table for the mapped points (the
    /// composition root feeds these into
    /// [`crate::adapters::acl::AntiCorruptionLayer::new`]). Empty until a
    /// discovery succeeds.
    pub fn mapped_point_mappings(&self) -> Vec<PointMapping> {
        let mut mappings = Vec::new();
        for entry in &self.register {
            if matches!(entry.status, ChannelMappingStatus::Mapped) {
                if let Some(meta) = self.mapped.get(&entry.point) {
                    mappings.push(PointMapping {
                        point: meta.point.clone(),
                        asset_key: meta.asset_key.clone(),
                        quantity: meta.quantity,
                        native_unit: meta.native_unit,
                        native_sign: meta.native_sign,
                        reactive_source_convention: meta.reactive_convention,
                    });
                }
            }
        }
        mappings
    }

    /// The points the profile explicitly declares unmapped (the ACL's
    /// declared-unmapped inventory).
    pub fn declared_unmapped_points(&self) -> Vec<PointKey> {
        self.register
            .iter()
            .filter(|entry| {
                matches!(entry.status, ChannelMappingStatus::ExplicitlyUnmapped { .. })
            })
            .map(|entry| entry.point.clone())
            .collect()
    }

    /// Classifies ONE discovered channel against the profile (the
    /// fail-closed rules of the module header).
    fn classify_channel(
        &self,
        point: &PointKey,
        source_declared_unit: Option<&str>,
    ) -> (ChannelMappingStatus, Option<MappedPointMeta>) {
        let matches: Vec<&ProfileMapping> = self
            .profile
            .mappings()
            .iter()
            .filter(|m| m.component_id == point.component && m.channel_id == point.channel)
            .collect();
        let declared = self
            .profile
            .declared_unmapped()
            .iter()
            .find(|u| u.component_id == point.component && u.channel_id == point.channel);

        if let (Some(_), Some(um)) = (declared, matches.first()) {
            let _ = um;
            return (
                ChannelMappingStatus::Ambiguous {
                    reason: "the profile declares this point BOTH mapped and unmapped"
                        .to_string(),
                },
                None,
            );
        }
        if let Some(um) = declared {
            return (
                ChannelMappingStatus::ExplicitlyUnmapped {
                    reason: um.reason.clone(),
                },
                None,
            );
        }
        match matches.len() {
            0 => (
                ChannelMappingStatus::Unmapped {
                    reason: "no mapping-profile v1 entry for this point; refusing to map \
                             by guessing"
                        .to_string(),
                },
                None,
            ),
            1 => match self.validate_entry(matches[0], point, source_declared_unit) {
                Ok(meta) => (ChannelMappingStatus::Mapped, Some(meta)),
                Err(reason) => (ChannelMappingStatus::Ambiguous { reason }, None),
            },
            count => (
                ChannelMappingStatus::Ambiguous {
                    reason: format!(
                        "the profile carries {count} mapping entries for this point \
                         (duplicate declarations are ambiguous)"
                    ),
                },
                None,
            ),
        }
    }

    /// Validates ONE profile entry against the declared-unit/sign rules and
    /// the source-declared unit. Every failure is an AMBIGUOUS reason —
    /// never a guessed mapping.
    fn validate_entry(
        &self,
        mapping: &ProfileMapping,
        point: &PointKey,
        source_declared_unit: Option<&str>,
    ) -> Result<MappedPointMeta, String> {
        let quantity = parse_quantity(&mapping.quantity).ok_or_else(|| {
            format!(
                "unknown quantity spelling '{}' (expected ActivePower|ReactivePower|\
                 Energy|Soc)",
                mapping.quantity
            )
        })?;
        let native_unit =
            NativeUnit::parse(&mapping.native_unit).map_err(|_| {
                format!(
                    "unknown native unit '{}' (the unit table names exactly what it \
                     accepts; no invented unit, no default scale)",
                    mapping.native_unit
                )
            })?;

        // Cross-check the source-declared unit (UNVERIFIED-ASSUMPTION key)
        // against the profile declaration. Conflict OR unnameable source
        // unit that differs from the profile spelling → AMBIGUOUS.
        if let Some(source_unit) = source_declared_unit.map(str::trim).filter(|s| !s.is_empty())
        {
            match NativeUnit::parse(source_unit) {
                Ok(source) if source != native_unit => {
                    return Err(format!(
                        "the source-declared unit '{source_unit}' conflicts with the \
                         profile-declared unit '{}'; refusing to map an ambiguous unit",
                        mapping.native_unit
                    ));
                }
                Err(_) if source_unit != mapping.native_unit => {
                    return Err(format!(
                        "the source-declared unit '{source_unit}' is not in the named \
                         unit table and differs from the profile unit '{}'; refusing to \
                         map by guessing",
                        mapping.native_unit
                    ));
                }
                _ => {}
            }
        }

        // Unit/quantity pairing (P0.07; mirrored from the ACL's rule so a
        // mismatched entry is flagged here, at discovery).
        let expected_family = match quantity {
            TelemetryQuantity::ActivePower => "power",
            TelemetryQuantity::ReactivePower => "reactive",
            TelemetryQuantity::Energy => "energy",
            TelemetryQuantity::Soc => "soc",
            _ => {
                return Err(format!(
                    "quantity '{}' has no named unit mapping in this adapter",
                    mapping.quantity
                ))
            }
        };
        if unit_family(native_unit) != expected_family {
            return Err(format!(
                "native unit '{}' cannot map to quantity '{}' (unit/quantity pairing, \
                 P0.07)",
                mapping.native_unit, mapping.quantity
            ));
        }

        let mut native_sign = None;
        let mut reactive_convention = None;
        match quantity {
            TelemetryQuantity::ActivePower => {
                let spelling = mapping.native_sign.as_deref().ok_or_else(|| {
                    "an active-power mapping REQUIRES a declared native sign meaning \
                     (P0.07 §2); refusing an undeclared sign"
                        .to_string()
                })?;
                native_sign = Some(parse_native_sign(spelling).ok_or_else(|| {
                    format!("unknown native sign spelling '{spelling}'")
                })?);
            }
            TelemetryQuantity::ReactivePower => {
                let spelling = match &mapping.reactive_source_convention {
                    Some(override_spelling) => override_spelling.as_str(),
                    None => convention_spelling(self.profile.reactive_source_convention()),
                };
                reactive_convention =
                    Some(parse_convention(spelling).ok_or_else(|| {
                        format!("unknown reactive convention spelling '{spelling}'")
                    })?);
            }
            _ => {}
        }

        Ok(MappedPointMeta {
            point: point.clone(),
            asset_key: mapping.asset_key.clone(),
            quantity,
            native_unit,
            native_sign,
            reactive_convention,
            declared_quality_code: mapping
                .declared_quality_code
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string),
            sequence: 0,
        })
    }

    /// The discovery body: fetch + parse the EdgeConfig, classify every
    /// channel, build the register and the mapped-point set.
    fn discover_inner(&mut self) -> Result<Vec<ChannelDescriptor>, AdapterError> {
        let response = self
            .transport
            .get(OPENEMS_EDGE_CONFIG_PATH)
            .map_err(|error| AdapterError::DiscoveryFailed {
                reason: error.to_string(),
            })?;
        if response.status != 200 {
            return Err(AdapterError::DiscoveryFailed {
                reason: OpenemsError::HttpStatus {
                    status: response.status,
                    path: OPENEMS_EDGE_CONFIG_PATH.to_string(),
                }
                .to_string(),
            });
        }
        // UNVERIFIED-ASSUMPTION — EdgeConfig response shape (components as
        // an object keyed by component id; each component carries alias +
        // channels; each channel carries an optional unit string). Verify
        // against the lab Edge (RESEARCH.md §1; report §27 OpenEMS
        // references).
        let document = json::parse(&response.body).map_err(|error| {
            AdapterError::DiscoveryFailed {
                reason: format!("the EdgeConfig response is not valid JSON: {error}"),
            }
        })?;
        let components = document
            .get("components")
            .and_then(|value| value.as_object())
            .ok_or_else(|| AdapterError::DiscoveryFailed {
                reason: "the EdgeConfig response carries no 'components' object \
                         (UNVERIFIED-ASSUMPTION shape; the lab Edge must be checked)"
                    .to_string(),
            })?;

        let mut entries: Vec<ChannelMappingEntry> = Vec::new();
        let mut metas: HashMap<PointKey, MappedPointMeta> = HashMap::new();
        for (component_id, component) in components {
            if !is_rest_path_safe(component_id) {
                entries.push(ChannelMappingEntry {
                    point: PointKey::new(component_id.clone(), "<unaddressable>"),
                    status: ChannelMappingStatus::Unmapped {
                        reason: "the component id is not REST-path-safe; its channels \
                                 cannot be addressed"
                            .to_string(),
                    },
                    source_declared_unit: None,
                    description: format!("OpenEMS component '{component_id}'"),
                });
                continue;
            }
            // UNVERIFIED-ASSUMPTION — the 'alias' and 'channels' field
            // spellings.
            let alias = component
                .get("alias")
                .and_then(|value| value.as_str())
                .unwrap_or(component_id);
            let Some(channels) = component.get("channels").and_then(|v| v.as_object()) else {
                continue;
            };
            for (channel_id, channel) in channels {
                if !is_rest_path_safe(channel_id) {
                    entries.push(ChannelMappingEntry {
                        point: PointKey::new(component_id.clone(), channel_id.clone()),
                        status: ChannelMappingStatus::Unmapped {
                            reason: "the channel id is not REST-path-safe; the channel \
                                     cannot be addressed"
                                .to_string(),
                        },
                        source_declared_unit: None,
                        description: format!("OpenEMS channel '{channel_id}'"),
                    });
                    continue;
                }
                let point = PointKey::new(component_id.clone(), channel_id.clone());
                // UNVERIFIED-ASSUMPTION — the 'unit' field spelling.
                let source_declared_unit = channel
                    .get("unit")
                    .and_then(|value| value.as_str())
                    .map(str::trim)
                    .filter(|s| !s.is_empty());
                let description = format!(
                    "OpenEMS channel '{channel_id}' on component '{component_id}' \
                     (component alias '{alias}')"
                );
                let (status, meta) = self.classify_channel(&point, source_declared_unit);
                entries.push(ChannelMappingEntry {
                    point: point.clone(),
                    status,
                    source_declared_unit: source_declared_unit.map(str::to_string),
                    description,
                });
                if let Some(meta) = meta {
                    metas.insert(point, meta);
                }
            }
        }

        // Deterministic register order (by point string) — the register is
        // a reviewed artifact, not a hash-map accident.
        entries.sort_by(|a, b| a.point.to_string().cmp(&b.point.to_string()));
        let descriptors: Vec<ChannelDescriptor> = entries
            .iter()
            .filter(|entry| matches!(entry.status, ChannelMappingStatus::Mapped))
            .filter_map(|entry| {
                let meta = metas.get(&entry.point)?;
                Some(ChannelDescriptor {
                    point: entry.point.clone(),
                    native_unit: meta.native_unit,
                    description: entry.description.clone(),
                })
            })
            .collect();

        self.register = entries;
        self.mapped = metas;
        self.discovered = true;
        Ok(descriptors)
    }
}

impl EmsAdapter for OpenEmsRestAdapter {
    fn id(&self) -> &str {
        OPENEMS_REST_ADAPTER_ID
    }

    fn capabilities(&self) -> &AdapterCapabilities {
        &self.capabilities
    }

    fn discover(&mut self) -> Result<Vec<ChannelDescriptor>, AdapterError> {
        match self.discover_inner() {
            Ok(descriptors) => {
                self.health = AdapterHealth::Healthy;
                Ok(descriptors)
            }
            Err(error) => {
                self.health = AdapterHealth::Unreachable {
                    reason: format!("EdgeConfig discovery failed: {error}"),
                };
                Err(error)
            }
        }
    }

    fn read(&mut self, point: &PointKey) -> Result<EmsNativeReading, AdapterError> {
        if !self.discovered {
            return Err(AdapterError::NotConnected {
                reason: "EdgeConfig discovery has not succeeded yet; the adapter refuses \
                         to fabricate a reading"
                    .to_string(),
            });
        }
        let Some(meta) = self.mapped.get_mut(point) else {
            return Err(AdapterError::ReadFailed {
                point: point.clone(),
                reason: "the point has no mapping-profile v1 entry (unmapped points are \
                         refused, never guessed)"
                    .to_string(),
            });
        };
        // Both ids were vetted at discovery (REST-path-safe), so the path
        // carries no separators beyond the two the format expects.
        let path = format!(
            "{prefix}{component}/{channel}",
            prefix = OPENEMS_CHANNEL_PATH_PREFIX,
            component = meta.point.component,
            channel = meta.point.channel,
        );
        let response = self.transport.get(&path).map_err(|error| {
            self.health = AdapterHealth::Unreachable {
                reason: format!("the channel read failed at the transport: {error}"),
            };
            AdapterError::NotConnected {
                reason: format!("the channel read request failed: {error}"),
            }
        })?;
        if response.status != 200 {
            return Err(AdapterError::ReadFailed {
                point: point.clone(),
                reason: OpenemsError::HttpStatus {
                    status: response.status,
                    path,
                }
                .to_string(),
            });
        }
        // UNVERIFIED-ASSUMPTION — channel-value response shape: an object
        // with a numeric 'value' and a numeric 'timestamp' (Unix epoch
        // milliseconds). Verify against the lab Edge; a mismatch is a
        // named ReadFailed, never a guessed interpretation.
        let document =
            json::parse(&response.body).map_err(|error| AdapterError::ReadFailed {
                point: point.clone(),
                reason: format!("the channel response is not valid JSON: {error}"),
            })?;
        let Some(value) = document.get("value").and_then(|v| v.as_f64()) else {
            return Err(AdapterError::ReadFailed {
                point: point.clone(),
                reason: "the channel response carries no numeric 'value' field \
                         (UNVERIFIED-ASSUMPTION response shape)"
                    .to_string(),
            });
        };
        if !value.is_finite() {
            return Err(AdapterError::ReadFailed {
                point: point.clone(),
                reason: "the channel value is not a finite number".to_string(),
            });
        }
        let Some(timestamp) = document.get("timestamp").and_then(|v| v.as_f64()) else {
            return Err(AdapterError::ReadFailed {
                point: point.clone(),
                reason: "the channel response carries no 'timestamp' field; event_time \
                         is never fabricated"
                    .to_string(),
            });
        };
        if !timestamp.is_finite() || timestamp < 0.0 || timestamp.fract() != 0.0 {
            return Err(AdapterError::ReadFailed {
                point: point.clone(),
                reason: "the channel timestamp is not a non-negative whole number of \
                         milliseconds"
                    .to_string(),
            });
        }
        let timestamp_millis = timestamp as i64;

        // The freshness limit: a reading older than the profile's declared
        // stale_after is stamped with the frozen-table code STALE (mapped
        // to canonical STALE by the ACL). A FRESH reading carries the
        // entry's DECLARED quality code; an entry with no declaration
        // yields NO code — the ACL refuses it (no default quality).
        let now = self.clock.now_ingest_time().unix_millis();
        let fresh = now.saturating_sub(timestamp_millis)
            <= self.profile.stale_after().as_millis() as i64;
        let quality_code = if fresh {
            meta.declared_quality_code.clone()
        } else {
            Some(OPENEMS_STALE_CODE.to_string())
        };

        meta.sequence += 1;
        Ok(EmsNativeReading {
            point: point.clone(),
            native_unit: meta.native_unit,
            raw_value: value,
            native_quality_code: quality_code,
            event_time: NativeTimestamp::UnixMillis(timestamp_millis),
            source_sequence: meta.sequence,
        })
    }

    fn health(&self) -> AdapterHealth {
        self.health.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use flexcore_contracts::time_model::ClockQuality;

    // --- JSON reader --------------------------------------------------------

    #[test]
    fn json_parses_the_shapes_the_openems_surfaces_use() {
        let parsed = json::parse(
            r#"{"components": {"ess0": {"alias": "battery", "channels": {"Soc": {"unit": "%"}}}},
                "n": [1, -2.5e2, true, null, "x\tA\ud83d\ude00"]}"#,
        )
        .expect("parses");
        let ess0 = parsed
            .get("components")
            .and_then(|c| c.get("ess0"))
            .expect("ess0 present");
        assert_eq!(ess0.get("alias").and_then(|v| v.as_str()), Some("battery"));
        assert_eq!(
            ess0
                .get("channels")
                .and_then(|c| c.get("Soc"))
                .and_then(|c| c.get("unit"))
                .and_then(|v| v.as_str()),
            Some("%")
        );
        let items = parsed.get("n").and_then(|v| v.as_array()).expect("array");
        assert_eq!(items.len(), 5);
        assert_eq!(items[1].as_f64(), Some(-250.0));
        assert_eq!(items[4].as_str(), Some("x\tA\u{1F600}"));
    }

    #[test]
    fn json_refuses_trailing_garbage_control_chars_and_deep_nesting() {
        assert!(json::parse("{} x").is_err());
        assert!(json::parse("{\"a\": \"\u{0001}\"}").is_err());
        assert!(json::parse("{\"a\": 1,").is_err());
        let deep = "[".repeat(70) + &"]".repeat(70);
        assert!(json::parse(&deep).is_err(), "the depth bound holds");
    }

    // --- Base URL policy ------------------------------------------------------

    #[test]
    fn base_url_accepts_http_and_refuses_everything_else_fail_closed() {
        let transport =
            TcpHttpGetTransport::from_base_url("http://lab-edge:8084", Duration::from_secs(5), Duration::from_secs(10));
        assert!(transport.is_ok());
        let transport = transport.expect("ok");
        assert_eq!(transport.host(), "lab-edge");
        assert_eq!(transport.port(), 8084);

        // Default port.
        let transport = TcpHttpGetTransport::from_base_url(
            "http://lab-edge",
            Duration::from_secs(5),
            Duration::from_secs(10),
        )
        .expect("ok");
        assert_eq!(transport.port(), 80);

        // A TLS endpoint is REFUSED (the REST-hop TLS client is a recorded
        // open item — never a silent downgrade).
        assert!(matches!(
            TcpHttpGetTransport::from_base_url(
                "https://lab-edge",
                Duration::from_secs(5),
                Duration::from_secs(10)
            ),
            Err(OpenemsError::BaseUrl { .. })
        ));
        assert!(matches!(
            TcpHttpGetTransport::from_base_url(
                "ftp://lab-edge",
                Duration::from_secs(5),
                Duration::from_secs(10)
            ),
            Err(OpenemsError::BaseUrl { .. })
        ));
    }

    // --- Profile envelope -----------------------------------------------------

    fn minimal_profile_json() -> String {
        format!(
            r#"{{
                "profile": "{PROFILE_ID}",
                "version": {SUPPORTED_PROFILE_VERSION},
                "reactive_source_convention": "injection-positive",
                "cadence_millis": 5000,
                "stale_after_millis": 15000,
                "mappings": [
                    {{"component_id": "ess0", "channel_id": "Soc", "asset_key": "ess-01",
                      "quantity": "Soc", "native_unit": "%", "declared_quality_code": "GOOD"}}
                ]
            }}"#
        )
    }

    #[test]
    fn profile_parses_and_carries_assumptions() {
        let text = minimal_profile_json();
        let profile = OpenemsMappingProfile::from_json_str(&text).expect("parses");
        assert_eq!(profile.version(), SUPPORTED_PROFILE_VERSION);
        assert_eq!(profile.cadence(), Duration::from_millis(5000));
        assert_eq!(profile.stale_after(), Duration::from_millis(15000));
        assert_eq!(profile.mappings().len(), 1);
    }

    #[test]
    fn profile_envelope_failures_are_named() {
        // Wrong identity.
        let wrong_identity = minimal_profile_json().replacen(
            &format!("\"profile\": \"{PROFILE_ID}\""),
            "\"profile\": \"something-else\"",
            1,
        );
        assert!(matches!(
            OpenemsMappingProfile::from_json_str(&wrong_identity),
            Err(OpenemsError::ProfileEnvelope { .. })
        ));
        // Unsupported version.
        let wrong_version = minimal_profile_json()
            .replacen(&format!("\"version\": {SUPPORTED_PROFILE_VERSION}"), "\"version\": 2", 1);
        assert!(matches!(
            OpenemsMappingProfile::from_json_str(&wrong_version),
            Err(OpenemsError::ProfileEnvelope { .. })
        ));
        // Missing required field.
        let missing = minimal_profile_json()
            .replacen("\"cadence_millis\": 5000,", "");
        assert!(matches!(
            OpenemsMappingProfile::from_json_str(&missing),
            Err(OpenemsError::ProfileEnvelope { .. })
        ));
        // Not JSON at all.
        assert!(matches!(
            OpenemsMappingProfile::from_json_str("not json"),
            Err(OpenemsError::Json { .. })
        ));
    }

    // --- Declaration spellings --------------------------------------------------

    #[test]
    fn declaration_spellings_parse_or_refuse() {
        assert_eq!(
            parse_quantity("ActivePower"),
            Some(TelemetryQuantity::ActivePower)
        );
        assert_eq!(parse_quantity("Soc"), Some(TelemetryQuantity::Soc));
        assert_eq!(parse_quantity("Flurb"), None);
        assert_eq!(
            parse_native_sign("import-positive"),
            Some(NativeSign::ImportPositive)
        );
        assert_eq!(parse_native_sign("positive"), None);
        assert_eq!(
            parse_convention("injection-positive"),
            Some(ReactiveSignConvention::InjectionPositive)
        );
        assert_eq!(parse_convention("Q-positive"), None);
    }

    #[test]
    fn rest_path_safety_rejects_traversal_shapes() {
        assert!(is_rest_path_safe("ess0"));
        assert!(is_rest_path_safe("_meta"));
        assert!(!is_rest_path_safe(""));
        assert!(!is_rest_path_safe("a/b"));
        assert!(!is_rest_path_safe("a b"));
        assert!(!is_rest_path_safe("a?b"));
        assert!(!is_rest_path_safe(".."));
    }

    #[test]
    fn capabilities_declare_unknown_clock_quality_and_the_stale_code() {
        // The adapter's declared quality mapping always contains the
        // adapter-derived STALE code, and the clock quality is UNKNOWN
        // until the lab run records it (P0.09 §4).
        let profile =
            OpenemsMappingProfile::from_json_str(&minimal_profile_json()).expect("parses");
        let adapter = OpenEmsRestAdapter::new(
            Box::new(NoopTransport),
            profile,
            Arc::new(flexcore_ports::fakes::FixedClock::at_unix_millis(1_000)),
        )
        .expect("composes");
        let capabilities = adapter.capabilities();
        assert_eq!(capabilities.adapter_id, OPENEMS_REST_ADAPTER_ID);
        assert_eq!(capabilities.clock_quality, ClockQuality::Unknown);
        let codes: Vec<&str> = capabilities
            .quality_mapping
            .mappings()
            .iter()
            .map(|m| m.source_code.as_str())
            .collect();
        assert!(codes.contains(&OPENEMS_STALE_CODE));
        assert!(codes.contains(&"GOOD"));
    }

    /// A transport that never answers (used only to compose the adapter in
    /// tests that never perform I/O).
    struct NoopTransport;

    impl OpenemsRestTransport for NoopTransport {
        fn get(&mut self, _path: &str) -> Result<RestResponse, OpenemsError> {
            Err(OpenemsError::Transport {
                reason: "no transport in this test".to_string(),
            })
        }
    }
}
