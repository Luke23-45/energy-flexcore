// FIM-F13.03 — ISO 15118 Plug & Charge OCSP validation path (code surface):
// a hand-written MINIMAL DER reader (the sha1.rs hand-codec precedent) and
// the fail-closed OCSP status extraction the charging-authorization refusal
// consumes.
//
// HONESTY HEADER (FIM-F13.06; FIM standing decision 1 and 2):
//
// - **Implemented-under-FIM-F13, CODE SURFACE ONLY.** The owner authorized
//   the CODE SURFACE of feature F-13 (ledger_F13.md). No OCSP responder is
//   operated, no OCSP query was ever made, no revocation status was ever
//   established. OCSP/DER conformance against a real responder is a HUMAN
//   LAB GATE and is UNPROVEN.
// - **The five record-41 §3 ungate conditions are NOT met and are NOT
//   claimed met** (see `crate::pnc_certificate_flows`); the OCSP path is
//   gated on the SAME five-condition record and refuses while any is
//   UNMET, and additionally refuses fail-closed when no responder is
//   configured. Each condition is a HUMAN GATE.
// - **WRITTEN-UNRUN**: never compiled, never executed; execution is a
//   HUMAN LAB GATE (conventions §1.3; the FIM hard boundary).
// - **Hand-written minimal DER reader (the sha1.rs precedent):** the
//   committed `Cargo.lock` resolves no DER/X.509 crate usable without a
//   new dependency (FIM standing decision 1: no new crate enters the
//   resolution graph), so the reader is implemented HERE as a small
//   pinned module. It supports ONLY what the pinned OCSP subset needs:
//   definite-length TLVs (short and two-byte long form), low tag numbers,
//   constructed/universal/context classes. Indefinite lengths, high tag
//   numbers, and wider long forms are REFUSED (fail-closed, never guessed
//   through). The fixtures pinning it are hand-built byte arrays with
//   python-computed expected values (tests/pnc_fim_ocsp.rs) — they pin the
//   reader, they are NOT real OCSP traffic.
// - **The pinned subset (recorded honestly):** the reader extracts an OCSP
//   response's certificate status (good / revoked / unknown) for a
//   requested serial and verifies the response against the EXPECTED
//   responder certificate by BYTE EQUALITY of the embedded certificate's
//   full DER (a reference equality against injected configuration — the
//   responder certificate arrives through [`OcspResponderConfig`], never
//   bundled). **NO full X.509 parsing and NO signature verification and NO
//   chain building happen here**: the certID hash fields, the signature
//   algorithm, and the signature bits are carried, never checked. Real
//   chain building and signature verification are DEPLOYMENT REQUIREMENTS
//   and HUMAN GATES (P14.03 interplay: no CA / no PKI is stood up in this
//   program). A mismatch at the OCA/RFC-text verification gate (record 41
//   §3.4) is a code or record edit, never a silent reinterpretation.
// - **Fail-closed, never fail-open:** a revoked certificate, an unknown
//   status, an unreachable responder, a malformed response, a responder
//   mismatch, and a serial with no status ALL refuse the charging
//   authorization by name. There is no code path that maps an absent or
//   unverifiable status to a pass (the P14.02/P17.03 fail-closed DEFAULT
//   carries over; record 41 §2).

use crate::pnc_certificate_flows::{PncGateCondition, PncGateConditions};

/// The sanity upper bound for one OCSP response (a gate against unbounded
/// input, NOT a protocol size claim).
pub const MAX_OCSP_RESPONSE_BYTES: usize = 65_536;

/// The RFC 6960 responseStatus value for `successful` (published RFC 6960
/// numbering, carried UNVERIFIED against the RFC text — no primary source
/// in the repository; record 41 §3.4 owns the check).
pub const OCSP_RESPONSE_STATUS_SUCCESSFUL: u8 = 0;

/// The DER content bytes of the id-pkix-ocsp-basic response type OID
/// (1.3.6.1.5.5.7.48.1.1; published RFC 6960 numbering, UNVERIFIED against
/// the RFC text — record 41 §3.4 owns the check).
const OID_ID_PKIX_OCSP_BASIC: [u8; 9] = [0x2B, 0x06, 0x01, 0x05, 0x05, 0x07, 0x30, 0x01, 0x01];

// --- The minimal DER reader (hand-written, pinned subset) -------------------------

/// One parsed DER TLV element (the pinned subset: definite lengths, low
/// tag numbers).
#[derive(Debug, Clone, PartialEq, Eq)]
struct DerElement<'a> {
    tag: u8,
    constructed: bool,
    /// The element's content bytes.
    content: &'a [u8],
    /// The FULL element bytes (header + content) — the byte-equality
    /// ranges are cut from this.
    full: &'a [u8],
}

/// Reads one DER TLV element at `offset` (fail-closed: every unsupported
/// DER feature is a named [`OcspResponseError::MalformedResponse`], never
/// a guess). Returns the element.
fn read_element(bytes: &[u8], offset: usize) -> Result<DerElement<'_>, OcspResponseError> {
    if offset >= bytes.len() {
        return Err(OcspResponseError::MalformedResponse {
            reason: "unexpected end of DER".to_string(),
        });
    }
    let tag = bytes[offset];
    if tag & 0x1F == 0x1F {
        return Err(OcspResponseError::MalformedResponse {
            reason: "multi-byte (high) tag numbers are outside the pinned subset".to_string(),
        });
    }
    let mut pos = offset + 1;
    if pos >= bytes.len() {
        return Err(OcspResponseError::MalformedResponse {
            reason: "DER element ends before its length".to_string(),
        });
    }
    let first_len_byte = bytes[pos];
    pos += 1;
    let length = match first_len_byte {
        len if len < 0x80 => len as usize,
        0x80 => {
            return Err(OcspResponseError::MalformedResponse {
                reason: "indefinite-length DER is refused (definite lengths only)".to_string(),
            })
        }
        0x81 => {
            if pos >= bytes.len() {
                return Err(OcspResponseError::MalformedResponse {
                    reason: "DER length bytes truncated".to_string(),
                });
            }
            let len = bytes[pos] as usize;
            pos += 1;
            len
        }
        0x82 => {
            if pos + 1 >= bytes.len() {
                return Err(OcspResponseError::MalformedResponse {
                    reason: "DER length bytes truncated".to_string(),
                });
            }
            let len = ((bytes[pos] as usize) << 8) | bytes[pos + 1] as usize;
            pos += 2;
            len
        }
        _ => {
            return Err(OcspResponseError::MalformedResponse {
                reason: "long-form length wider than two bytes is outside the pinned subset"
                    .to_string(),
            })
        }
    };
    if length > bytes.len() - pos {
        return Err(OcspResponseError::MalformedResponse {
            reason: "DER length claims more bytes than remain".to_string(),
        });
    }
    Ok(DerElement {
        tag,
        constructed: tag & 0x20 != 0,
        content: &bytes[pos..pos + length],
        full: &bytes[offset..pos + length],
    })
}

/// Reads the successive children of a constructed element's content.
fn read_children(content: &[u8]) -> Result<Vec<DerElement<'_>>, OcspResponseError> {
    let mut children = Vec::new();
    let mut offset = 0;
    while offset < content.len() {
        let element = read_element(content, offset)?;
        offset += element.full.len();
        children.push(element);
    }
    Ok(children)
}

/// Parses a GeneralizedTime limited to the pinned `YYYYMMDDHHMMSSZ` form.
/// Fractional seconds, offsets, and local forms are REFUSED (fail-closed,
/// never approximated).
fn parse_generalized_time_millis(content: &[u8]) -> Result<i64, OcspResponseError> {
    let malformed = || OcspResponseError::MalformedResponse {
        reason: "GeneralizedTime is outside the pinned YYYYMMDDHHMMSSZ subset".to_string(),
    };
    if content.len() != 15 || content[14] != b'Z' {
        return Err(malformed());
    }
    let mut digits = [0u32; 14];
    for (index, byte) in content[..14].iter().enumerate() {
        if !byte.is_ascii_digit() {
            return Err(malformed());
        }
        digits[index] = (byte - b'0') as u32;
    }
    let year = (digits[0] * 1000 + digits[1] * 100 + digits[2] * 10 + digits[3]) as i64;
    let month = (digits[4] * 10 + digits[5]) as u32;
    let day = (digits[6] * 10 + digits[7]) as u32;
    let hour = (digits[8] * 10 + digits[9]) as i64;
    let minute = (digits[10] * 10 + digits[11]) as i64;
    let second = (digits[12] * 10 + digits[13]) as i64;
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return Err(malformed());
    }
    let days = days_from_civil(year, month, day);
    Ok((days * 86_400 + hour * 3_600 + minute * 60 + second) * 1_000)
}

/// Days from 1970-01-01 to the proleptic-Gregorian date (the standard
/// days-from-civil formulation; pinned against python-computed vectors in
/// the tests).
fn days_from_civil(year: i64, month: u32, day: u32) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = if year >= 0 { year } else { year - 399 } / 400;
    let year_of_era = year - era * 400;
    let month_position = ((month + 9) % 12) as i64;
    let day_of_year = (153 * month_position + 2) / 5 + day as i64 - 1;
    let day_of_era =
        year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

// --- The OCSP model (pinned subset) ------------------------------------------------

/// The OCSP certificate status of one SingleResponse (the pinned subset:
/// the three RFC 6960 certStatus choices; UNVERIFIED against the RFC text
/// — record 41 §3.4 owns the check).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OcspCertificateStatus {
    /// certStatus good ([0] IMPLICIT NULL).
    Good,
    /// certStatus revoked ([1] IMPLICIT RevokedInfo): the revocation
    /// instant (pinned GeneralizedTime form) and the CRLReason code, when
    /// the response carries the optional reason.
    Revoked {
        revocation_time_millis: i64,
        reason_code: Option<u8>,
    },
    /// certStatus unknown ([2] IMPLICIT UnknownInfo) — fail-closed: never
    /// a pass.
    Unknown,
}

impl OcspCertificateStatus {
    /// The stable machine-readable status name.
    pub const fn name(&self) -> &'static str {
        match self {
            OcspCertificateStatus::Good => "OCSP_STATUS_GOOD",
            OcspCertificateStatus::Revoked { .. } => "OCSP_STATUS_REVOKED",
            OcspCertificateStatus::Unknown => "OCSP_STATUS_UNKNOWN",
        }
    }
}

/// One parsed OCSP response (the pinned subset's extraction).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OcspParsedResponse {
    /// The FULL DER of the FIRST certificate in the response's certs
    /// element, when present — the byte-equality-by-reference input for
    /// the responder verification. `None` = the response carries no
    /// responder certificate (fail-closed: unverified responders are
    /// refused).
    pub embedded_responder_certificate_der: Option<Vec<u8>>,
    /// The status of the SingleResponse whose certID serial bytes equal
    /// the requested serial, when one matches. `None` = no match
    /// (fail-closed: an absent status is never a pass).
    pub requested_status: Option<OcspCertificateStatus>,
}

/// Parses one OCSPResponse for the pinned subset: a successful
/// responseStatus, an id-pkix-ocsp-basic responseBytes, and the status of
/// the SingleResponse matching `serial_der_content` (the DER INTEGER
/// content bytes of the serial — exact content-byte equality, no
/// big-integer arithmetic).
pub fn parse_ocsp_response(
    response_der: &[u8],
    serial_der_content: &[u8],
) -> Result<OcspParsedResponse, OcspResponseError> {
    if response_der.is_empty() {
        return Err(OcspResponseError::MalformedResponse {
            reason: "the OCSP response is empty".to_string(),
        });
    }
    if response_der.len() > MAX_OCSP_RESPONSE_BYTES {
        return Err(OcspResponseError::MalformedResponse {
            reason: "the OCSP response exceeds the sanity bound".to_string(),
        });
    }
    if serial_der_content.is_empty() {
        return Err(OcspResponseError::MalformedResponse {
            reason: "the requested serial content is empty".to_string(),
        });
    }
    let outer = read_element(response_der, 0)?;
    if !(outer.tag == 0x30 && outer.constructed) {
        return Err(OcspResponseError::MalformedResponse {
            reason: "the OCSPResponse is not a SEQUENCE".to_string(),
        });
    }
    let top = read_children(outer.content)?;
    // responseStatus: ENUMERATED (the first child).
    let status_element = top.first().ok_or_else(|| OcspResponseError::MalformedResponse {
        reason: "the OCSPResponse carries no responseStatus".to_string(),
    })?;
    if status_element.tag != 0x0A || status_element.content.len() != 1 {
        return Err(OcspResponseError::MalformedResponse {
            reason: "the responseStatus is not a one-byte ENUMERATED".to_string(),
        });
    }
    let response_status = status_element.content[0];
    if response_status != OCSP_RESPONSE_STATUS_SUCCESSFUL {
        return Err(OcspResponseError::ResponseStatusUnsuccessful {
            code: response_status,
        });
    }
    // responseBytes [0] EXPLICIT (the second child, when present).
    let response_bytes = top.get(1).ok_or_else(|| OcspResponseError::MalformedResponse {
        reason: "a successful OCSPResponse carries no responseBytes (pinned subset)".to_string(),
    })?;
    if response_bytes.tag != 0xA0 {
        return Err(OcspResponseError::MalformedResponse {
            reason: "the responseBytes element is not the [0] EXPLICIT".to_string(),
        });
    }
    let response_bytes_children = read_children(response_bytes.content)?;
    if response_bytes_children.len() != 1 {
        return Err(OcspResponseError::MalformedResponse {
            reason: "the responseBytes [0] EXPLICIT must carry exactly one ResponseBytes \
                     SEQUENCE"
                .to_string(),
        });
    }
    let response_bytes_sequence = &response_bytes_children[0];
    if !(response_bytes_sequence.tag == 0x30 && response_bytes_sequence.constructed) {
        return Err(OcspResponseError::MalformedResponse {
            reason: "the ResponseBytes is not a SEQUENCE".to_string(),
        });
    }
    let response_bytes_fields = read_children(response_bytes_sequence.content)?;
    if response_bytes_fields.len() != 2 {
        return Err(OcspResponseError::MalformedResponse {
            reason: "the ResponseBytes must carry exactly responseType and response"
                .to_string(),
        });
    }
    let response_type = &response_bytes_fields[0];
    if response_type.tag != 0x06 || response_type.content != OID_ID_PKIX_OCSP_BASIC {
        return Err(OcspResponseError::UnsupportedResponseType);
    }
    let response_octets = &response_bytes_fields[1];
    if response_octets.tag != 0x04 {
        return Err(OcspResponseError::MalformedResponse {
            reason: "the response element is not an OCTET STRING".to_string(),
        });
    }
    // BasicOCSPResponse: SEQUENCE { tbsResponseData, signatureAlgorithm,
    // signature, [0] certs OPTIONAL }.
    let basic = read_element(response_octets.content, 0)?;
    if !(basic.tag == 0x30 && basic.constructed) {
        return Err(OcspResponseError::MalformedResponse {
            reason: "the BasicOCSPResponse is not a SEQUENCE".to_string(),
        });
    }
    let basic_children = read_children(basic.content)?;
    if basic_children.len() < 3 {
        return Err(OcspResponseError::MalformedResponse {
            reason: "the BasicOCSPResponse lacks its required elements".to_string(),
        });
    }
    let tbs = &basic_children[0];
    if !(tbs.tag == 0x30 && tbs.constructed) {
        return Err(OcspResponseError::MalformedResponse {
            reason: "the tbsResponseData is not a SEQUENCE".to_string(),
        });
    }
    // The embedded responder certificate: the FIRST certificate of the
    // optional [0] certs element ([0] EXPLICIT wraps a SEQUENCE OF
    // Certificate, so the unwrap is two levels) — the
    // byte-equality-by-reference input. NO X.509 parsing happens — that
    // is the recorded limitation.
    let embedded_responder_certificate_der = basic_children
        .iter()
        .find(|element| element.tag == 0xA0)
        .and_then(|certs_element| {
            read_children(certs_element.content)
                .ok()
                .and_then(|explicit_children| {
                    explicit_children
                        .iter()
                        .find(|element| element.tag == 0x30)
                        .and_then(|sequence_of| read_children(sequence_of.content).ok())
                })
                .and_then(|certificates| {
                    certificates
                        .iter()
                        .find(|element| element.tag == 0x30)
                        .map(|certificate| certificate.full.to_vec())
                })
        });
    // ResponseData: skip the optional [0] version; responderID ([1]/[2]);
    // producedAt (GeneralizedTime); responses (SEQUENCE OF
    // SingleResponse). The pinned subset walks the order and refuses
    // anything else.
    let tbs_children = read_children(tbs.content)?;
    if tbs_children.len() < 3 {
        return Err(OcspResponseError::MalformedResponse {
            reason: "the tbsResponseData lacks its required elements".to_string(),
        });
    }
    let mut position = 0;
    if tbs_children[position].tag == 0xA0 {
        position += 1; // the optional [0] EXPLICIT version
    }
    let responder_id = tbs_children
        .get(position)
        .ok_or_else(|| OcspResponseError::MalformedResponse {
            reason: "the tbsResponseData carries no responderID".to_string(),
        })?;
    if responder_id.tag != 0xA1 && responder_id.tag != 0xA2 {
        return Err(OcspResponseError::MalformedResponse {
            reason: "the responderID is not the [1]/[2] EXPLICIT".to_string(),
        });
    }
    position += 1;
    let produced_at = tbs_children
        .get(position)
        .ok_or_else(|| OcspResponseError::MalformedResponse {
            reason: "the tbsResponseData carries no producedAt".to_string(),
        })?;
    if produced_at.tag != 0x18 {
        return Err(OcspResponseError::MalformedResponse {
            reason: "the producedAt is not a GeneralizedTime".to_string(),
        });
    }
    parse_generalized_time_millis(produced_at.content)?;
    position += 1;
    let responses = tbs_children
        .get(position)
        .ok_or_else(|| OcspResponseError::MalformedResponse {
            reason: "the tbsResponseData carries no responses element".to_string(),
        })?;
    if !(responses.tag == 0x30 && responses.constructed) {
        return Err(OcspResponseError::MalformedResponse {
            reason: "the responses element is not a SEQUENCE".to_string(),
        });
    }
    // Walk the SingleResponses; the FIRST matching certID serial wins.
    let mut requested_status = None;
    for single in read_children(responses.content)? {
        if !(single.tag == 0x30 && single.constructed) {
            return Err(OcspResponseError::MalformedResponse {
                reason: "a SingleResponse is not a SEQUENCE".to_string(),
            });
        }
        let single_children = read_children(single.content)?;
        if single_children.len() < 3 {
            return Err(OcspResponseError::MalformedResponse {
                reason: "a SingleResponse lacks its required elements".to_string(),
            });
        }
        let cert_id = &single_children[0];
        if !(cert_id.tag == 0x30 && cert_id.constructed) {
            return Err(OcspResponseError::MalformedResponse {
                reason: "the CertID is not a SEQUENCE".to_string(),
            });
        }
        let cert_id_children = read_children(cert_id.content)?;
        let serial = cert_id_children.iter().rev().find(|element| element.tag == 0x02);
        let serial_matches = serial
            .map(|element| element.content == serial_der_content)
            .unwrap_or(false);
        if !serial_matches {
            continue;
        }
        let cert_status = &single_children[1];
        let status = match cert_status.tag {
            // good [0] IMPLICIT NULL — primitive, empty content
            0x80 => OcspCertificateStatus::Good,
            // revoked [1] IMPLICIT RevokedInfo — constructed SEQUENCE
            0xA1 => {
                let revoked_info = read_element(cert_status.content, 0)?;
                if !(revoked_info.tag == 0x30 && revoked_info.constructed) {
                    return Err(OcspResponseError::MalformedResponse {
                        reason: "the RevokedInfo is not a SEQUENCE".to_string(),
                    });
                }
                let revoked_children = read_children(revoked_info.content)?;
                let revocation_time = revoked_children.first().ok_or_else(|| {
                    OcspResponseError::MalformedResponse {
                        reason: "the RevokedInfo carries no revocationTime".to_string(),
                    }
                })?;
                if revocation_time.tag != 0x18 {
                    return Err(OcspResponseError::MalformedResponse {
                        reason: "the revocationTime is not a GeneralizedTime".to_string(),
                    });
                }
                let revocation_time_millis =
                    parse_generalized_time_millis(revocation_time.content)?;
                let reason_code = revoked_children.get(1).and_then(|reason_element| {
                    // revocationReason [0] EXPLICIT CRLReason
                    if reason_element.tag != 0xA0 {
                        return None;
                    }
                    read_element(reason_element.content, 0)
                        .ok()
                        .filter(|enumerated| enumerated.tag == 0x0A)
                        .and_then(|enumerated| enumerated.content.first().copied())
                });
                OcspCertificateStatus::Revoked {
                    revocation_time_millis,
                    reason_code,
                }
            }
            // unknown [2] IMPLICIT UnknownInfo — primitive, empty content
            0x82 => OcspCertificateStatus::Unknown,
            _ => {
                return Err(OcspResponseError::MalformedResponse {
                    reason: "the certStatus is outside the pinned good/revoked/unknown subset"
                        .to_string(),
                })
            }
        };
        requested_status = Some(status);
        break;
    }
    Ok(OcspParsedResponse {
        embedded_responder_certificate_der,
        requested_status,
    })
}

// --- The responder configuration and transport seam --------------------------------

/// The OCSP responder configuration (FIM-F13.03): injected CONFIGURATION
/// — the responder URL reference comes from the deployment's configuration,
/// and the expected responder certificate DER comes from the deployment's
/// trust anchor management. Nothing is bundled, discovered, or guessed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OcspResponderConfig {
    /// The responder URL reference (the deployment's configured OCSP
    /// service endpoint reference; carried as configuration data — this
    /// module performs no network I/O itself).
    pub responder_url_reference: String,
    /// The EXPECTED responder certificate DER bytes (injected; the pinned
    /// byte-equality-by-reference target — NO X.509 parsing, NO signature
    /// verification, NO chain building).
    pub expected_responder_certificate_der: Vec<u8>,
}

impl OcspResponderConfig {
    /// Validates the injected configuration (fail-closed: an empty URL
    /// reference or an empty expected certificate is refused — an
    /// unconfigured responder must fail the OCSP path loudly, never
    /// silently skip it).
    pub fn new(
        responder_url_reference: String,
        expected_responder_certificate_der: Vec<u8>,
    ) -> Result<Self, OcspResponseError> {
        if responder_url_reference.trim().is_empty() {
            return Err(OcspResponseError::ResponderNotConfigured);
        }
        if expected_responder_certificate_der.is_empty() {
            return Err(OcspResponseError::ResponderNotConfigured);
        }
        Ok(Self {
            responder_url_reference,
            expected_responder_certificate_der,
        })
    }
}

/// The OCSP query record (the transport's input; a REFERENCE shape — this
/// module composes no network bytes itself).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OcspRequest {
    /// The requested certificate's serial, as the DER INTEGER content
    /// bytes (exact content-byte equality against the response's certID).
    pub serial_der_content: Vec<u8>,
    /// The query instant (the injected clock; recorded data).
    pub created_at_millis: i64,
}

impl OcspRequest {
    /// Builds a validated query (an empty serial is refused).
    pub fn for_serial(serial_der_content: Vec<u8>, created_at_millis: i64) -> Self {
        Self {
            serial_der_content,
            created_at_millis,
        }
    }
}

/// The OCSP transport seam: the deployment implements the actual
/// responder exchange (this program performs no network I/O). Sync trait,
/// house rules. An unreachable responder is `Err` — the OCSP path treats
/// that as a fail-closed refusal, NEVER a pass.
pub trait OcspTransport {
    /// The transport name for audit lines.
    fn name(&self) -> &'static str;
    /// Fetches the raw OCSPResponse DER from the configured responder for
    /// one query. `Err` = unreachable/failed (fail-closed).
    fn fetch(
        &self,
        responder: &OcspResponderConfig,
        request: &OcspRequest,
    ) -> Result<Vec<u8>, OcspTransportError>;
}

/// The named transport failures (the seam's own error type; mapped to the
/// fail-closed [`OcspResponseError::ResponderUnreachable`] by the client).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OcspTransportError {
    /// The responder could not be reached (network down, timeout, DNS
    /// failure — the reason is human-safe).
    Unreachable { reason: String },
}

impl OcspTransportError {
    /// The stable machine-readable error name.
    pub fn name(&self) -> &'static str {
        match self {
            OcspTransportError::Unreachable { .. } => "PNC_OCSP_TRANSPORT_UNREACHABLE",
        }
    }
}

impl std::fmt::Display for OcspTransportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OcspTransportError::Unreachable { reason } => {
                write!(f, "the OCSP responder is unreachable: {reason}")
            }
        }
    }
}

impl std::error::Error for OcspTransportError {}

// --- The client --------------------------------------------------------------------

/// The OCSP client (FIM-F13.03): composes the injected transport seam with
/// the injected responder configuration and the five-condition gate record.
/// EVERY failure refuses the charging authorization; only a verified
/// `good` status permits.
pub struct OcspClient<'a> {
    transport: &'a dyn OcspTransport,
    config: OcspResponderConfig,
}

impl<'a> OcspClient<'a> {
    /// Composes the client (the configuration is validated at
    /// construction; an unconfigured responder refuses loudly here, before
    /// any query).
    pub fn new(
        transport: &'a dyn OcspTransport,
        config: OcspResponderConfig,
    ) -> Result<Self, OcspResponseError> {
        Ok(Self {
            transport,
            config,
        })
    }

    /// The configured responder's URL reference (diagnostics).
    pub fn responder_url_reference(&self) -> &str {
        &self.config.responder_url_reference
    }

    /// Runs one OCSP check for `serial_der_content` at the injected
    /// instant `now_unix_millis` (never wall time):
    ///
    /// 1. the five record-41 §3 gates must ALL be recorded met (refusing
    ///    with the named unmet condition — FIM-F13.04);
    /// 2. the transport fetch must succeed (an unreachable responder is
    ///    the named fail-closed refusal — NEVER a pass);
    /// 3. the response must parse within the pinned subset;
    /// 4. the embedded responder certificate must BYTE-EQUAL the injected
    ///    expected responder certificate (reference equality — NO X.509
    ///    parsing, NO signature verification, NO chain building);
    /// 5. a SingleResponse must match the requested serial (an absent
    ///    status is never a pass).
    pub fn check(
        &self,
        gates: &PncGateConditions,
        serial_der_content: &[u8],
        now_unix_millis: i64,
    ) -> Result<OcspCertificateStatus, OcspResponseError> {
        if let Err(error) = gates.require_all_met() {
            return Err(match error {
                crate::pnc_certificate_flows::PncCertificateError::GateConditionUnmet {
                    condition,
                } => OcspResponseError::GateConditionUnmet { condition },
                other => OcspResponseError::GateEvaluationRefused {
                    reason: other.to_string(),
                },
            });
        }
        let request = OcspRequest::for_serial(serial_der_content.to_vec(), now_unix_millis);
        let raw = self.transport.fetch(&self.config, &request).map_err(|error| {
            OcspResponseError::ResponderUnreachable {
                reason: error.to_string(),
            }
        })?;
        let parsed = parse_ocsp_response(&raw, serial_der_content)?;
        let embedded = parsed.embedded_responder_certificate_der.ok_or(
            OcspResponseError::ResponderCertificateAbsent,
        )?;
        if embedded != self.config.expected_responder_certificate_der {
            return Err(OcspResponseError::ResponderCertificateMismatch);
        }
        parsed
            .requested_status
            .ok_or(OcspResponseError::NoStatusForSerial)
    }

    /// The charging-authorization decision (FIM-F13.03's consuming rule):
    /// `Ok(Permitted)` ONLY on a verified `good` status; revoked, unknown,
    /// unreachable, malformed, mismatched, and absent statuses are ALL the
    /// named `Err` refusals. The caller refuses charging on ANY `Err`.
    pub fn check_charging_authorization(
        &self,
        gates: &PncGateConditions,
        serial_der_content: &[u8],
        now_unix_millis: i64,
    ) -> Result<OcspChargingAuthorization, OcspResponseError> {
        match self.check(gates, serial_der_content, now_unix_millis)? {
            OcspCertificateStatus::Good => Ok(OcspChargingAuthorization::Permitted),
            OcspCertificateStatus::Revoked {
                revocation_time_millis,
                reason_code,
            } => Err(OcspResponseError::CertificateRevoked {
                revocation_time_millis,
                reason_code,
            }),
            OcspCertificateStatus::Unknown => Err(OcspResponseError::StatusUnknown),
        }
    }
}

/// The charging-authorization outcome of a verified OCSP check.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OcspChargingAuthorization {
    /// The certificate's status is verified `good`: the caller MAY permit
    /// the charging authorization (the caller's other authorization gates
    /// are its own; this decision is the revocation leg only).
    Permitted,
}

impl OcspChargingAuthorization {
    /// Canonical name for records.
    pub const fn name(self) -> &'static str {
        match self {
            OcspChargingAuthorization::Permitted => "OCSP_CHARGING_PERMITTED",
        }
    }
}

// --- Errors -----------------------------------------------------------------------

/// The named OCSP-path errors. EVERY variant refuses the charging
/// authorization — there is no fail-open path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OcspResponseError {
    /// The response did not parse within the pinned DER subset.
    MalformedResponse { reason: String },
    /// The responseStatus was not `successful` (the published RFC 6960
    /// numbering; UNVERIFIED against the RFC text).
    ResponseStatusUnsuccessful { code: u8 },
    /// The responseType was not id-pkix-ocsp-basic.
    UnsupportedResponseType,
    /// The response embeds no responder certificate to verify against.
    ResponderCertificateAbsent,
    /// The embedded responder certificate does not BYTE-EQUAL the injected
    /// expected responder certificate.
    ResponderCertificateMismatch,
    /// No SingleResponse matched the requested serial.
    NoStatusForSerial,
    /// The certificate's status is `revoked`: the charging authorization
    /// is REFUSED.
    CertificateRevoked {
        revocation_time_millis: i64,
        reason_code: Option<u8>,
    },
    /// The certificate's status is `unknown`: the charging authorization
    /// is REFUSED (never a pass).
    StatusUnknown,
    /// The responder could not be reached: the charging authorization is
    /// REFUSED (fail-closed — never fail-open).
    ResponderUnreachable { reason: String },
    /// The responder configuration is absent/incomplete (injected config
    /// missing — fail-closed).
    ResponderNotConfigured,
    /// A record-41 §3 ungate condition is UNMET (the named condition).
    GateConditionUnmet { condition: PncGateCondition },
    /// The gate record itself refused for another reason.
    GateEvaluationRefused { reason: String },
}

impl OcspResponseError {
    /// The stable machine-readable error name.
    pub fn name(&self) -> &'static str {
        match self {
            OcspResponseError::MalformedResponse { .. } => "PNC_OCSP_RESPONSE_MALFORMED",
            OcspResponseError::ResponseStatusUnsuccessful { .. } => {
                "PNC_OCSP_RESPONSE_STATUS_UNSUCCESSFUL"
            }
            OcspResponseError::UnsupportedResponseType => "PNC_OCSP_RESPONSE_TYPE_UNSUPPORTED",
            OcspResponseError::ResponderCertificateAbsent => "PNC_OCSP_RESPONDER_CERT_ABSENT",
            OcspResponseError::ResponderCertificateMismatch => {
                "PNC_OCSP_RESPONDER_CERT_MISMATCH"
            }
            OcspResponseError::NoStatusForSerial => "PNC_OCSP_NO_STATUS_FOR_SERIAL",
            OcspResponseError::CertificateRevoked { .. } => "PNC_OCSP_CERTIFICATE_REVOKED",
            OcspResponseError::StatusUnknown => "PNC_OCSP_STATUS_UNKNOWN",
            OcspResponseError::ResponderUnreachable { .. } => "PNC_OCSP_RESPONDER_UNREACHABLE",
            OcspResponseError::ResponderNotConfigured => "PNC_OCSP_RESPONDER_NOT_CONFIGURED",
            OcspResponseError::GateConditionUnmet { .. } => "PNC_GATE_CONDITION_UNMET",
            OcspResponseError::GateEvaluationRefused { .. } => "PNC_OCSP_GATE_EVALUATION_REFUSED",
        }
    }
}

impl std::fmt::Display for OcspResponseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OcspResponseError::MalformedResponse { reason } => {
                write!(f, "the OCSP response is malformed: {reason}")
            }
            OcspResponseError::ResponseStatusUnsuccessful { code } => {
                write!(f, "the OCSP responseStatus is not successful (code {code}); the \
                          charging authorization is refused (fail-closed)")
            }
            OcspResponseError::UnsupportedResponseType => {
                write!(f, "the OCSP responseType is not id-pkix-ocsp-basic; refused, never \
                          parsed through")
            }
            OcspResponseError::ResponderCertificateAbsent => {
                write!(f, "the OCSP response embeds no responder certificate to verify \
                          against; refused (fail-closed)")
            }
            OcspResponseError::ResponderCertificateMismatch => {
                write!(f, "the embedded responder certificate does not byte-equal the \
                          injected expected responder certificate; refused (fail-closed)")
            }
            OcspResponseError::NoStatusForSerial => {
                write!(f, "no SingleResponse in the OCSP response matches the requested \
                          serial; an absent status is never a pass (fail-closed)")
            }
            OcspResponseError::CertificateRevoked {
                revocation_time_millis,
                reason_code,
            } => write!(
                f,
                "the certificate is REVOKED (revocation instant {revocation_time_millis}, \
                 reason code {}) — the charging authorization is refused",
                reason_code
                    .map(|code| code.to_string())
                    .unwrap_or_else(|| "not reported".to_string()),
            ),
            OcspResponseError::StatusUnknown => {
                write!(f, "the certificate's OCSP status is unknown — the charging \
                          authorization is refused (fail-closed, never a pass)")
            }
            OcspResponseError::ResponderUnreachable { reason } => write!(
                f,
                "the OCSP responder is unreachable ({reason}) — the charging authorization \
                 is refused (fail-closed, never fail-open)"
            ),
            OcspResponseError::ResponderNotConfigured => {
                write!(f, "no OCSP responder is configured (injected configuration is \
                          mandatory) — the OCSP path refuses (fail-closed)")
            }
            OcspResponseError::GateConditionUnmet { condition } => write!(
                f,
                "the record-41 §3 ungate condition '{}' is UNMET — the OCSP path refuses \
                 while any condition is unmet (the conditions are NOT claimed met; this \
                 is a HUMAN GATE: {})",
                condition.name(),
                condition.human_gate_statement(),
            ),
            OcspResponseError::GateEvaluationRefused { reason } => {
                write!(f, "the gate record refused: {reason}")
            }
        }
    }
}

impl std::error::Error for OcspResponseError {}

#[cfg(test)]
mod tests {
    use super::*;

    const BASE: i64 = 1_700_000_000_000;

    #[test]
    fn generalized_time_parses_the_pinned_form_only() {
        // python-computed: 2026-01-01T00:00:00Z.
        assert_eq!(
            parse_generalized_time_millis(b"20260101000000Z").expect("parses"),
            1_767_225_600_000
        );
        assert_eq!(
            parse_generalized_time_millis(b"20260615231245Z").expect("parses"),
            1_781_565_165_000
        );
        // Fractional seconds, offsets, short forms: refused, never approximated.
        assert!(parse_generalized_time_millis(b"20260101000000.000Z").is_err());
        assert!(parse_generalized_time_millis(b"20260101T000000Z").is_err());
        assert!(parse_generalized_time_millis(b"20260101000000+0000").is_err());
        assert!(parse_generalized_time_millis(b"2026010100000Z").is_err());
        assert!(parse_generalized_time_millis(b"20261301000000Z").is_err());
    }

    #[test]
    fn the_der_reader_refuses_everything_outside_the_pinned_subset() {
        // Truncated length: the header claims more bytes than remain.
        let mut truncated = vec![0x30, 0x82, 0x00, 0xC8];
        truncated.extend_from_slice(&[0x01u8; 10]);
        assert!(matches!(
            read_element(&truncated, 0),
            Err(OcspResponseError::MalformedResponse { .. })
        ));
        // Indefinite length (0x80) is refused.
        assert!(matches!(
            read_element(&[0x30, 0x80, 0x00, 0x00], 0),
            Err(OcspResponseError::MalformedResponse { .. })
        ));
        // High tag numbers are refused.
        assert!(matches!(
            read_element(&[0x9F, 0x02, 0x01, 0x00], 0),
            Err(OcspResponseError::MalformedResponse { .. })
        ));
        // Three-byte long form is refused.
        assert!(matches!(
            read_element(&[0x30, 0x83, 0x00, 0x00, 0x01, 0x00], 0),
            Err(OcspResponseError::MalformedResponse { .. })
        ));
        // Unexpected end.
        assert!(matches!(
            read_element(&[0x30], 0),
            Err(OcspResponseError::MalformedResponse { .. })
        ));
        assert!(matches!(
            read_element(&[], 0),
            Err(OcspResponseError::MalformedResponse { .. })
        ));
    }

    #[test]
    fn the_pinned_subset_parser_refuses_the_named_failures() {
        // responseStatus != successful (published RFC 6960 numbering; the
        // fixture is SEQUENCE { ENUMERATED 6 } — unauthorized(6)).
        let unauthorized = [0x30, 0x03, 0x0A, 0x01, 0x06];
        assert_eq!(
            parse_ocsp_response(&unauthorized, &[0x01]).unwrap_err().name(),
            "PNC_OCSP_RESPONSE_STATUS_UNSUCCESSFUL"
        );
        // An empty response and an empty serial are refused.
        assert!(parse_ocsp_response(&[], &[0x01]).is_err());
        assert!(parse_ocsp_response(&unauthorized, &[]).is_err());
    }

    #[test]
    fn responder_configuration_is_fail_closed() {
        assert_eq!(
            OcspResponderConfig::new("  ".to_string(), vec![1])
                .unwrap_err()
                .name(),
            "PNC_OCSP_RESPONDER_NOT_CONFIGURED"
        );
        assert_eq!(
            OcspResponderConfig::new("responder-ref".to_string(), Vec::new())
                .unwrap_err()
                .name(),
            "PNC_OCSP_RESPONDER_NOT_CONFIGURED"
        );
        let config =
            OcspResponderConfig::new("responder-ref".to_string(), vec![1, 2, 3]).expect("valid");
        assert_eq!(config.responder_url_reference, "responder-ref");
        let _ = BASE;
    }
}
